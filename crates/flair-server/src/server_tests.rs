use super::*;
use flair_core::{
    config::Config,
    geom::{CellPoint, GridSize},
    key::{Key, KeyInput, Mods},
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
    assert!(server.pane.is_some());
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
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), ClientMsg::Resize(size)),
        &mut out,
    );
    assert!(
        out.iter().any(
            |effect| matches!(effect, Effect::ResizePty(PaneId(1), actual) if actual == &size)
        )
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { .. })))
    );
    let out = ack(&mut server, &initial);
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame { panes, .. }) if panes[0].full && panes[0].size == size)));
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
fn detach_shuts_down_owned_in_process_shell() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(ClientId(1), ClientMsg::Detach),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(PaneId(1))))
    );
    assert!(out.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::Exit(ExitReason::Detached))
    )));
    assert!(out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
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
            .pane
            .as_mut()
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
                point: CellPoint { col: 0, row: 0 },
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
