use super::*;
use flair_core::{
    command::Command,
    config::Config,
    geom::{CellPoint, GridSize},
    key::{InputEvent, Key, KeyAction, KeyInput, Locks, Mods, MouseAction, MouseButton},
    proto::ClientKind,
};

fn hello() -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        kind: ClientKind::Gui,
        size: GridSize { cols: 20, rows: 4 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target: Target::Main,
    }
}

fn setup() -> (Server, Vec<Effect>) {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    (server, out)
}

fn ack(server: &mut Server, effects: &[Effect]) -> Vec<Effect> {
    let mut out = Vec::new();
    for effect in effects {
        if let Effect::Send(client, ServerMsg::Frame { seq, .. }) = effect {
            server.handle(
                Millis(1),
                Event::Client(*client, ClientMsg::FrameAck { seq: *seq }),
                &mut out,
            );
        }
    }
    out
}

#[test]
fn hello_spawns_configured_shell_and_welcomes_client() {
    let (server, out) = setup();
    assert!(out.iter().any(|effect| matches!(effect, Effect::Spawn(PaneId(1), spec) if spec.size.cols == 20 && spec.env.iter().any(|(k,v)| k == "TERM_PROGRAM" && v == "flair"))));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Welcome { .. })))
    );
    assert!(!server.panes.is_empty());
}

#[test]
fn output_waits_for_matching_credit_and_preserves_merged_damage() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut first = Vec::new();
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"hi".to_vec()),
        &mut first,
    );
    assert!(first.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].rows[0].1[0].c == 'h')));
    let mut pending = Vec::new();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"there".to_vec()),
        &mut pending,
    );
    assert!(pending.is_empty());
    server.handle(
        Millis(4),
        Event::Client(ClientId(1), ClientMsg::FrameAck { seq: 999 }),
        &mut pending,
    );
    assert!(pending.is_empty());
    let merged = ack(&mut server, &first);
    assert!(merged.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].rows[0].1.iter().take(7).map(|cell| cell.c).collect::<String>() == "hithere")));
}

#[test]
fn control_c_is_encoded_for_the_pty() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(1),
            ClientMsg::Input(InputEvent::Key(KeyInput {
                key: Key::Char('c'),
                mods: Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                text: None,
                repeat: false,
                keypad: false,
                action: KeyAction::Press,
                unshifted: None,
                shifted: None,
                base: None,
                locks: Locks::default(),
            })),
        ),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(PaneId(1), bytes) if bytes == &[3]))
    );
}

#[test]
fn resize_preserves_credit_then_sends_full_frame() {
    let (mut server, initial) = setup();
    let mut out = Vec::new();
    let size = GridSize { cols: 30, rows: 6 };
    let pane_size = GridSize { cols: 30, rows: 5 };
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), ClientMsg::Resize(size)),
        &mut out,
    );
    assert!(out.iter().any(
        |effect| matches!(effect, Effect::ResizePty(PaneId(1), actual) if actual == &pane_size)
    ));
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { .. })))
    );
    let out = ack(&mut server, &initial);
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].full && panes[0].size == pane_size)));
}

#[test]
fn terminal_title_and_queries_become_effects() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"\x1b]2;title\x07\x1b[6n".to_vec()),
        &mut out,
    );
    assert!(out.iter().any(
        |effect| matches!(effect, Effect::Send(_, ServerMsg::Title(title)) if title == "title")
    ));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, bytes) if bytes.ends_with(b"R")))
    );
}

#[test]
fn detach_preserves_owned_shell() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), ClientMsg::Detach),
        &mut out,
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(_)))
    );
    assert!(out.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::Exit(ExitReason::Detached))
    )));
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn exited_shell_delivers_pending_output_before_exit() {
    let (mut server, initial) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"final".to_vec()),
        &mut out,
    );
    server.handle(Millis(3), Event::PtyExited(PaneId(1)), &mut out);
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
    let final_frame = ack(&mut server, &initial);
    assert!(
        final_frame
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { .. })))
    );
    let exit = ack(&mut server, &final_frame);
    assert!(exit.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::Exit(ExitReason::SessionEnded))
    )));
    assert!(exit.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn disconnected_client_does_not_stop_terminal_updates() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(Millis(2), Event::ClientGone(ClientId(1)), &mut out);
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"hidden".to_vec()),
        &mut out,
    );
    assert!(out.is_empty());
    assert!(
        server
            .panes
            .get_mut(&PaneId(1))
            .unwrap()
            .terminal
            .snapshot(PaneId(1), true)
            .unwrap()
            .rows[0]
            .1[0]
            .c
            == 'h'
    );
}

#[test]
fn bad_protocol_or_empty_geometry_never_spawns() {
    for message in [
        ClientMsg::Hello {
            protocol: PROTOCOL + 1,
            kind: ClientKind::Gui,
            size: GridSize { cols: 20, rows: 4 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        },
        ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 0, rows: 4 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        },
    ] {
        let mut server = Server::new(ServerConfig::from_config(&Config::default()));
        let mut out = Vec::new();
        server.handle(Millis(0), Event::Client(ClientId(1), message), &mut out);
        assert!(!out.iter().any(|effect| matches!(effect, Effect::Spawn(..))));
        assert!(
            out.iter()
                .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Error(_))))
        );
    }
}

#[test]
fn wheel_scrolls_history_and_typing_returns_to_bottom() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut output = Vec::new();
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7".to_vec()),
        &mut output,
    );
    ack(&mut server, &output);
    let mut scroll = Vec::new();
    server.handle(
        Millis(3),
        Event::Client(
            ClientId(1),
            ClientMsg::Input(InputEvent::Mouse(flair_core::key::MouseInput {
                point: CellPoint { col: 0, row: 1 },
                button: MouseButton::WheelUp,
                action: MouseAction::Press,
                mods: Mods::default(),
            })),
        ),
        &mut scroll,
    );
    assert!(scroll.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].full && panes[0].flags.display_offset > 0)));
    ack(&mut server, &scroll);
    let mut typed = Vec::new();
    server.handle(
        Millis(4),
        Event::Client(
            ClientId(1),
            ClientMsg::Input(InputEvent::Key(KeyInput {
                key: Key::Char('x'),
                mods: Mods::default(),
                text: None,
                repeat: false,
                keypad: false,
                action: KeyAction::Press,
                unshifted: None,
                shifted: None,
                base: None,
                locks: Locks::default(),
            })),
        ),
        &mut typed,
    );
    assert!(typed.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].flags.display_offset == 0)));
    assert!(
        typed
            .iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, bytes) if bytes == b"x"))
    );
}

#[test]
fn reattach_keeps_terminal_and_does_not_spawn_another_shell() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::ClientGone(ClientId(1)), &mut out);
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"retained".to_vec()),
        &mut out,
    );
    server.handle(Millis(3), Event::Client(ClientId(2), hello()), &mut out);
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Spawn(..))));
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(2), ServerMsg::Frame {panes,..}) if panes[0].rows[0].1[0].c == 'r')));
}

#[test]
fn two_clients_have_independent_credit_and_receive_output() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut second = Vec::new();
    server.handle(Millis(2), Event::Client(ClientId(2), hello()), &mut second);
    ack(&mut server, &second);
    let mut output = Vec::new();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"both".to_vec()),
        &mut output,
    );
    for id in [ClientId(1), ClientId(2)] {
        assert!(output.iter().any(|effect| matches!(effect, Effect::Send(client, ServerMsg::Frame {panes,..}) if *client == id && panes[0].rows[0].1[0].c == 'b')));
    }
}

#[test]
fn creating_a_hidden_session_does_not_change_attached_view() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(2),
            ClientMsg::Control(Control::Create {
                target: Target::Create {
                    name: Some("other".into()),
                    root: "/tmp".into(),
                },
                env: vec![],
            }),
        ),
        &mut out,
    );
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(1));
    assert!(out.iter().any(|effect| matches!(effect, Effect::Spawn(PaneId(2), spec) if spec.cwd == std::path::Path::new("/tmp"))));
    assert_eq!(server.tree.sessions.len(), 2);
}

#[test]
fn latest_active_client_decides_size_even_at_identical_time() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    let mut second = hello();
    if let ClientMsg::Hello { size, .. } = &mut second {
        size.cols = 40;
    }
    server.handle(Millis(0), Event::Client(ClientId(2), second), &mut out);
    server.handle(
        Millis(0),
        Event::Client(ClientId(1), ClientMsg::Focus(true)),
        &mut out,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 20);
    out.clear();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(2),
            ClientMsg::Resize(GridSize { cols: 50, rows: 4 }),
        ),
        &mut out,
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::ResizePty(..)))
    );
}

#[test]
fn no_history_session_explicitly_removes_inherited_histfile() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    let mut out = Vec::new();
    server.handle(
        Millis(0),
        Event::Client(
            ClientId(1),
            ClientMsg::Control(Control::Create {
                target: Target::Create {
                    name: Some("H".into()),
                    root: "/tmp".into(),
                },
                env: vec![("HISTFILE".into(), "/tmp/history".into())],
            }),
        ),
        &mut out,
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Spawn(_, spec) if spec.unset_env.iter().any(|key| key == "HISTFILE") && !spec.env.iter().any(|(key,_)| key == "HISTFILE"))));
}

fn key(character: char, ctrl: bool) -> ClientMsg {
    ClientMsg::Input(InputEvent::Key(KeyInput {
        key: Key::Char(character),
        mods: Mods {
            ctrl,
            ..Mods::default()
        },
        text: None,
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }))
}

#[test]
fn prefix_is_per_client_and_literal_prefix_reaches_pty() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    server.handle(Millis(2), Event::Client(ClientId(2), hello()), &mut out);
    server.handle(
        Millis(3),
        Event::Client(ClientId(1), key('a', true)),
        &mut out,
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Title(title)) if title.ends_with("^A"))));
    out.clear();
    server.handle(
        Millis(4),
        Event::Client(ClientId(2), key('f', false)),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, bytes) if bytes == b"f"))
    );
    server.handle(
        Millis(5),
        Event::Client(ClientId(1), key('a', true)),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, bytes) if bytes == &[1]))
    );
}

#[test]
fn prefix_sessionizer_reports_current_and_last_session() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(1),
            ClientMsg::Command(Command::Switch(Target::Path("/tmp/project".into()))),
        ),
        &mut out,
    );
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), key('a', true)),
        &mut out,
    );
    server.handle(
        Millis(3),
        Event::Client(ClientId(1), key('f', false)),
        &mut out,
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Sessionizer {sessions, current,last}) if sessions.len() == 2 && current == "project" && *last == Some(Target::Name("main".into())))));
    server.handle(
        Millis(4),
        Event::Client(ClientId(1), key('a', true)),
        &mut out,
    );
    server.handle(
        Millis(5),
        Event::Client(ClientId(1), key('l', false)),
        &mut out,
    );
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(1));
    assert_eq!(server.panes.len(), 2);
}

#[test]
fn switching_one_client_preserves_other_view_and_outstanding_credit() {
    let (mut server, initial) = setup();
    let original_seq = server.clients[&ClientId(1)].awaiting_ack;
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), hello()), &mut out);
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(1),
            ClientMsg::Command(Command::Switch(Target::Path("/tmp/other".into()))),
        ),
        &mut out,
    );
    assert_eq!(server.clients[&ClientId(2)].view.session, SessionId(1));
    assert_eq!(server.clients[&ClientId(1)].awaiting_ack, original_seq);
    let switched = ack(&mut server, &initial);
    assert!(switched.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1),ServerMsg::Frame{panes,..}) if panes[0].pane == PaneId(2) && panes[0].full)));
}

#[test]
fn exited_session_returns_client_to_its_last_session() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(1),
            ClientMsg::Command(Command::Switch(Target::Path("/tmp/other".into()))),
        ),
        &mut out,
    );
    ack(&mut server, &out);
    out.clear();
    server.handle(Millis(2), Event::PtyExited(PaneId(2)), &mut out);
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(1));
    assert_eq!(server.tree.sessions.len(), 1);
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn control_switch_targets_the_most_recent_gui_not_the_command_connection() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), hello()), &mut out);
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), ClientMsg::Focus(true)),
        &mut out,
    );
    server.handle(
        Millis(3),
        Event::Client(
            ClientId(99),
            ClientMsg::Control(Control::Switch {
                target: Target::Path("/tmp/focus".into()),
                pane: None,
                env: vec![],
            }),
        ),
        &mut out,
    );
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(2));
    assert_eq!(server.clients[&ClientId(2)].view.session, SessionId(1));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(99), ServerMsg::Done)))
    );
}

#[test]
fn sessionizer_prefix_expires_and_restores_title() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(ClientId(1), key('a', true)),
        &mut out,
    );
    out.clear();
    server.handle(Millis(1001), Event::Timer(1), &mut out);
    assert!(out.iter().any(
        |effect| matches!(effect,Effect::Send(_,ServerMsg::Title(title)) if !title.contains("^A"))
    ));
    server.handle(
        Millis(1002),
        Event::Client(ClientId(1), key('f', false)),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect,Effect::WritePty(_,bytes) if bytes == b"f"))
    );
}

#[test]
fn mouse_selection_sends_primary_and_clipboard_and_typing_clears_overlay() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::PtyOutput(PaneId(1), b"hello world".to_vec()),
        &mut out,
    );
    ack(&mut server, &out);
    out.clear();
    let mouse = |col, action| {
        ClientMsg::Input(InputEvent::Mouse(flair_core::key::MouseInput {
            point: CellPoint { col, row: 1 },
            button: MouseButton::Left,
            action,
            mods: Mods::default(),
        }))
    };
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), mouse(0, MouseAction::Press)),
        &mut out,
    );
    server.handle(
        Millis(3),
        Event::Client(ClientId(1), mouse(4, MouseAction::Motion)),
        &mut out,
    );
    server.handle(
        Millis(4),
        Event::Client(ClientId(1), mouse(4, MouseAction::Release)),
        &mut out,
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1),ServerMsg::SetClipboard {text,primary:true}) if text=="hello")));
    server.handle(
        Millis(5),
        Event::Client(ClientId(1), ClientMsg::Command(Command::CopySelection)),
        &mut out,
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1),ServerMsg::SetClipboard {text,primary:false}) if text=="hello")));
    server.handle(
        Millis(6),
        Event::Client(ClientId(1), key('x', false)),
        &mut out,
    );
    assert!(server.panes[&PaneId(1)].terminal.selection_text().is_none());
}

#[path = "server_windows_tests.rs"]
mod windows;

#[path = "server_split_tests.rs"]
mod splits;

#[path = "server_mouse_tests.rs"]
mod mouse;
