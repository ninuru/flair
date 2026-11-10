use super::*;
use flair_core::{
    command::{Command, CopyCommand},
    config::PromptStyle,
    geom::{CellPoint, GridSize},
    key::{
        InputEvent, Key, KeyAction, KeyInput, Locks, Mods, MouseAction, MouseButton, MouseInput,
        NamedKey,
    },
    proto::ClientKind,
    status::StatusLine,
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

fn setup() -> Server {
    let mut config = Config::default();
    config.prompt.style = PromptStyle::Ps1;
    let mut server = Server::new(ServerConfig::from_config(&config));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    ack(&mut server, &out);
    output(
        &mut server,
        b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\nhello",
    );
    server
}

fn ack(server: &mut Server, effects: &[Effect]) {
    for effect in effects {
        if let Effect::Send(client, ServerMsg::Frame { seq, .. }) = effect {
            let mut out = Vec::new();
            server.handle(
                Millis(1),
                Event::Client(*client, ClientMsg::FrameAck { seq: *seq }),
                &mut out,
            );
        }
    }
}

fn send(server: &mut Server, message: ClientMsg) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(10), Event::Client(ClientId(1), message), &mut out);
    ack(server, &out);
    out
}

fn output(server: &mut Server, bytes: &[u8]) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(10),
        Event::PtyOutput(PaneId(1), bytes.to_vec()),
        &mut out,
    );
    ack(server, &out);
    out
}

fn input(key: Key, mods: Mods, action: KeyAction) -> ClientMsg {
    ClientMsg::Input(InputEvent::Key(KeyInput {
        key,
        mods,
        text: None,
        repeat: false,
        keypad: false,
        action,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }))
}

fn press(server: &mut Server, key: Key) -> Vec<Effect> {
    send(server, input(key, Mods::default(), KeyAction::Press))
}

fn chord(server: &mut Server, key: Key, mods: Mods) -> Vec<Effect> {
    send(server, input(key, mods, KeyAction::Press))
}

fn ctrl() -> Mods {
    Mods {
        ctrl: true,
        ..Mods::default()
    }
}

fn enter(server: &mut Server) -> Vec<Effect> {
    chord(server, Key::Char('a'), ctrl());
    press(server, Key::Char('['))
}

fn mouse(server: &mut Server, row: u16, button: MouseButton, action: MouseAction) -> Vec<Effect> {
    send(
        server,
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 0, row },
            button,
            action,
            mods: Mods::default(),
        })),
    )
}

fn wheel(server: &mut Server, button: MouseButton) -> Vec<Effect> {
    mouse(server, 0, button, MouseAction::Press)
}

fn status(out: &[Effect]) -> Option<&StatusLine> {
    out.iter().find_map(|effect| {
        if let Effect::Send(ClientId(1), ServerMsg::Status(line)) = effect {
            Some(line)
        } else {
            None
        }
    })
}

fn writes(out: &[Effect]) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|effect| {
            if let Effect::WritePty(_, bytes) = effect {
                Some(bytes.as_slice())
            } else {
                None
            }
        })
        .collect()
}

fn copy_mode(server: &Server) -> bool {
    server.panes[&PaneId(1)].terminal.copy_mode()
}

fn offset(server: &Server) -> u32 {
    server.panes[&PaneId(1)]
        .cached
        .as_ref()
        .unwrap()
        .flags
        .display_offset
}

#[test]
fn prefix_bracket_enters_copy_mode_with_status_badge_and_frame_flag() {
    let mut server = setup();
    let out = enter(&mut server);
    assert!(copy_mode(&server));
    assert!(status(&out).unwrap().copy);
    assert!(writes(&out).is_empty());
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Frame { panes, .. }) if panes[0].flags.copy_mode)));
}

#[test]
fn q_and_escape_leave_copy_mode() {
    let mut server = setup();
    for key in [Key::Char('q'), Key::Named(NamedKey::Escape)] {
        enter(&mut server);
        let out = press(&mut server, key);
        assert!(!copy_mode(&server));
        assert!(!status(&out).unwrap().copy);
        assert!(writes(&out).is_empty());
    }
}

#[test]
fn unbound_keys_and_releases_never_reach_the_pty_in_copy_mode() {
    let mut server = setup();
    enter(&mut server);
    for message in [
        input(Key::Char('x'), Mods::default(), KeyAction::Press),
        input(Key::Char('x'), Mods::default(), KeyAction::Release),
        input(Key::Char('c'), ctrl(), KeyAction::Press),
        input(Key::Named(NamedKey::Tab), Mods::default(), KeyAction::Press),
        input(
            Key::Named(NamedKey::ShiftLeft),
            Mods::default(),
            KeyAction::Press,
        ),
    ] {
        assert!(writes(&send(&mut server, message)).is_empty());
    }
    chord(&mut server, Key::Char('a'), ctrl());
    assert!(writes(&chord(&mut server, Key::Char('a'), ctrl())).is_empty());
    assert!(copy_mode(&server));
}

#[test]
fn root_and_prefix_bindings_keep_working_in_copy_mode() {
    let mut server = setup();
    enter(&mut server);
    chord(&mut server, Key::Char('a'), ctrl());
    let out = press(&mut server, Key::Char('c'));
    assert!(writes(&out).is_empty());
    assert_eq!(server.tree.sessions[0].windows.len(), 2);
    assert!(!status(&out).unwrap().copy);
    assert!(copy_mode(&server));
    let out = chord(
        &mut server,
        Key::Char('1'),
        Mods {
            alt: true,
            ..Mods::default()
        },
    );
    assert!(status(&out).unwrap().copy);
}

#[test]
fn text_and_paste_leave_copy_mode_and_reach_the_pty() {
    let mut server = setup();
    for (message, expected) in [
        (InputEvent::Text("x".into()), &b"x"[..]),
        (InputEvent::Paste("hi".into()), &b"hi"[..]),
    ] {
        enter(&mut server);
        let out = send(&mut server, ClientMsg::Input(message));
        assert!(!copy_mode(&server));
        assert!(!status(&out).unwrap().copy);
        assert_eq!(writes(&out), vec![expected]);
    }
}

#[test]
fn click_leaves_copy_mode_and_still_starts_a_selection() {
    let mut server = setup();
    enter(&mut server);
    wheel(&mut server, MouseButton::WheelUp);
    assert!(offset(&server) > 0);
    let out = mouse(&mut server, 3, MouseButton::Left, MouseAction::Press);
    assert!(!copy_mode(&server));
    assert!(!status(&out).unwrap().copy);
    assert_eq!(offset(&server), 0);
    send(
        &mut server,
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 4, row: 3 },
            button: MouseButton::Left,
            action: MouseAction::Release,
            mods: Mods::default(),
        })),
    )
    .iter()
    .find(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::SetClipboard { text, .. }) if text == "hello"))
    .unwrap();
    for button in [MouseButton::Middle, MouseButton::Right] {
        enter(&mut server);
        mouse(&mut server, 0, button, MouseAction::Press);
        assert!(!copy_mode(&server));
    }
}

#[test]
fn wheel_up_enters_copy_mode_and_wheel_down_to_the_bottom_leaves_it() {
    let mut server = setup();
    let out = wheel(&mut server, MouseButton::WheelUp);
    assert!(copy_mode(&server));
    assert!(status(&out).unwrap().copy);
    assert_eq!(offset(&server), 3);
    wheel(&mut server, MouseButton::WheelUp);
    assert_eq!(offset(&server), 6);
    wheel(&mut server, MouseButton::WheelDown);
    assert!(copy_mode(&server));
    assert_eq!(offset(&server), 3);
    let out = wheel(&mut server, MouseButton::WheelDown);
    assert!(!copy_mode(&server));
    assert!(!status(&out).unwrap().copy);
    assert_eq!(offset(&server), 0);
}

#[test]
fn wheel_down_to_the_bottom_keeps_copy_mode_entered_by_the_prefix() {
    let mut server = setup();
    enter(&mut server);
    wheel(&mut server, MouseButton::WheelUp);
    wheel(&mut server, MouseButton::WheelDown);
    assert_eq!(offset(&server), 0);
    assert!(copy_mode(&server));
}

#[test]
fn wheel_in_copy_mode_scrolls_instead_of_reporting() {
    let mut server = setup();
    output(&mut server, b"\x1b[?1000h\x1b[?1006h");
    let out = wheel(&mut server, MouseButton::WheelUp);
    assert!(!writes(&out).is_empty());
    assert!(!copy_mode(&server));
    enter(&mut server);
    let out = wheel(&mut server, MouseButton::WheelUp);
    assert!(writes(&out).is_empty());
    assert_eq!(offset(&server), 3);
}

#[test]
fn wheel_on_the_alt_screen_does_not_enter_copy_mode() {
    let mut server = setup();
    output(&mut server, b"\x1b[?1049h");
    let out = wheel(&mut server, MouseButton::WheelUp);
    assert!(!copy_mode(&server));
    assert!(writes(&out).is_empty());
}

#[test]
fn resize_and_clear_leave_copy_mode() {
    let mut server = setup();
    enter(&mut server);
    let out = send(
        &mut server,
        ClientMsg::Resize(GridSize { cols: 30, rows: 5 }),
    );
    assert!(!copy_mode(&server));
    assert!(!status(&out).unwrap().copy);
    enter(&mut server);
    let out = output(&mut server, b"\x1b[2J");
    assert!(!copy_mode(&server));
    assert!(!status(&out).unwrap().copy);
}

fn shifted(server: &mut Server, c: char) -> Vec<Effect> {
    chord(
        server,
        Key::Char(c),
        Mods {
            shift: true,
            ..Mods::default()
        },
    )
}

fn cursor(server: &Server) -> CellPoint {
    server.panes[&PaneId(1)]
        .cached
        .as_ref()
        .unwrap()
        .cursor
        .point
}

fn selection(server: &Server) -> Option<String> {
    server.panes[&PaneId(1)].terminal.selection_text()
}

fn copy_setup() -> Server {
    let mut server = setup();
    output(&mut server, b"\r\nfoo bar");
    enter(&mut server);
    server
}

#[test]
fn motion_keys_including_shifted_ones_move_the_vi_cursor() {
    let mut server = copy_setup();
    let mut written = Vec::new();
    written.extend(press(&mut server, Key::Char('0')));
    assert_eq!(cursor(&server), CellPoint { col: 0, row: 3 });
    written.extend(shifted(&mut server, 'W'));
    assert_eq!(cursor(&server), CellPoint { col: 4, row: 3 });
    written.extend(shifted(&mut server, 'B'));
    assert_eq!(cursor(&server), CellPoint { col: 0, row: 3 });
    written.extend(shifted(&mut server, 'E'));
    assert_eq!(cursor(&server), CellPoint { col: 2, row: 3 });
    written.extend(shifted(&mut server, 'H'));
    assert_eq!(cursor(&server).row, 0);
    written.extend(shifted(&mut server, 'M'));
    assert_eq!(cursor(&server).row, 1);
    written.extend(shifted(&mut server, 'L'));
    assert_eq!(cursor(&server).row, 3);
    written.extend(press(&mut server, Key::Char('g')));
    assert!(offset(&server) > 0);
    assert_eq!(cursor(&server).row, 0);
    written.extend(shifted(&mut server, 'G'));
    assert_eq!(offset(&server), 0);
    assert_eq!(cursor(&server).row, 3);
    written.extend(chord(&mut server, Key::Char('u'), ctrl()));
    assert_eq!(offset(&server), 2);
    written.extend(chord(&mut server, Key::Char('d'), ctrl()));
    assert_eq!(offset(&server), 0);
    written.extend(press(&mut server, Key::Char('k')));
    assert_eq!(cursor(&server).row, 2);
    written.extend(press(&mut server, Key::Named(NamedKey::Down)));
    assert_eq!(cursor(&server).row, 3);
    assert!(writes(&written).is_empty());
    assert!(copy_mode(&server));
}

#[test]
fn selection_keys_select_convert_clear_and_swap() {
    let mut server = copy_setup();
    press(&mut server, Key::Char('0'));
    shifted(&mut server, 'V');
    assert!(selection(&server).unwrap().starts_with("foo bar"));
    chord(&mut server, Key::Char('v'), ctrl());
    assert_eq!(selection(&server).as_deref(), Some("f"));
    press(&mut server, Key::Char('v'));
    press(&mut server, Key::Char('l'));
    press(&mut server, Key::Char('l'));
    assert_eq!(selection(&server).as_deref(), Some("foo"));
    press(&mut server, Key::Char('o'));
    assert_eq!(cursor(&server), CellPoint { col: 0, row: 3 });
    assert_eq!(selection(&server).as_deref(), Some("foo"));
    press(&mut server, Key::Char('v'));
    assert_eq!(selection(&server), None);
}

#[test]
fn search_keys_prompt_the_client_and_search_runs_from_the_command() {
    let mut server = copy_setup();
    let out = press(&mut server, Key::Char('/'));
    assert!(out.iter().any(|effect| matches!(
        effect,
        Effect::Send(ClientId(1), ServerMsg::PromptSearch { forward: true })
    )));
    let out = shifted(&mut server, '?');
    assert!(out.iter().any(|effect| matches!(
        effect,
        Effect::Send(ClientId(1), ServerMsg::PromptSearch { forward: false })
    )));
    assert!(writes(&out).is_empty());
    press(&mut server, Key::Char('0'));
    send(
        &mut server,
        ClientMsg::Command(Command::Copy(CopyCommand::Search {
            pattern: "o".into(),
            forward: false,
        })),
    );
    assert_eq!(cursor(&server), CellPoint { col: 4, row: 2 });
    press(&mut server, Key::Char('n'));
    assert_eq!(cursor(&server), CellPoint { col: 2, row: 3 });
    shifted(&mut server, 'N');
    assert_eq!(cursor(&server), CellPoint { col: 4, row: 2 });
}

#[test]
fn invalid_search_is_ignored_without_an_error() {
    let mut server = copy_setup();
    let before = cursor(&server);
    let out = send(
        &mut server,
        ClientMsg::Command(Command::Copy(CopyCommand::Search {
            pattern: "(".into(),
            forward: true,
        })),
    );
    assert!(!out.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::Error(_) | ServerMsg::Exit(_))
    )));
    assert_eq!(cursor(&server), before);
    assert!(copy_mode(&server));
}

#[test]
fn copy_commands_outside_copy_mode_do_nothing() {
    let mut server = setup();
    let out = send(
        &mut server,
        ClientMsg::Command(Command::Copy(CopyCommand::SearchPrompt { forward: true })),
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptSearch { .. })))
    );
}

fn clipboard(out: &[Effect]) -> Option<(&str, bool)> {
    out.iter().find_map(|effect| {
        if let Effect::Send(ClientId(1), ServerMsg::SetClipboard { text, primary }) = effect {
            Some((text.as_str(), *primary))
        } else {
            None
        }
    })
}

fn ctrl_shift_c(server: &mut Server) -> Vec<Effect> {
    chord(
        server,
        Key::Char('c'),
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
    )
}

#[test]
fn y_and_enter_yank_the_selection_to_the_clipboard_and_exit() {
    let mut server = setup();
    for key in [Key::Char('y'), Key::Named(NamedKey::Enter)] {
        enter(&mut server);
        press(&mut server, Key::Char('0'));
        press(&mut server, Key::Char('v'));
        press(&mut server, Key::Char('e'));
        let out = press(&mut server, key);
        assert_eq!(clipboard(&out), Some(("hello", false)));
        assert!(writes(&out).is_empty());
        assert!(!copy_mode(&server));
        assert!(!status(&out).unwrap().copy);
        assert_eq!(selection(&server), None);
    }
}

#[test]
fn yank_without_a_selection_only_exits() {
    let mut server = setup();
    enter(&mut server);
    let out = press(&mut server, Key::Char('y'));
    assert_eq!(clipboard(&out), None);
    assert!(!copy_mode(&server));
}

#[test]
fn ctrl_shift_c_copies_the_vi_selection_and_stays_in_copy_mode() {
    let mut server = setup();
    enter(&mut server);
    press(&mut server, Key::Char('0'));
    press(&mut server, Key::Char('v'));
    press(&mut server, Key::Char('l'));
    let out = ctrl_shift_c(&mut server);
    assert_eq!(clipboard(&out), Some(("he", false)));
    assert!(writes(&out).is_empty());
    assert!(copy_mode(&server));
    assert_eq!(selection(&server).as_deref(), Some("he"));
}

#[test]
fn drag_after_the_exiting_click_selects_for_primary_and_ctrl_shift_c() {
    let mut server = setup();
    enter(&mut server);
    mouse(&mut server, 3, MouseButton::Left, MouseAction::Press);
    assert!(!copy_mode(&server));
    let out = send(
        &mut server,
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 3, row: 3 },
            button: MouseButton::Left,
            action: MouseAction::Motion,
            mods: Mods::default(),
        })),
    );
    assert_eq!(clipboard(&out), None);
    let out = send(
        &mut server,
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 3, row: 3 },
            button: MouseButton::Left,
            action: MouseAction::Release,
            mods: Mods::default(),
        })),
    );
    assert_eq!(clipboard(&out), Some(("hell", true)));
    assert_eq!(clipboard(&ctrl_shift_c(&mut server)), Some(("hell", false)));
}

#[test]
fn alt_screen_copy_mode_stays_on_the_visible_screen_and_yanks_it() {
    let mut server = setup();
    output(
        &mut server,
        b"\x1b[?1049h\x1b[Halt0\r\nalt1\r\nalt2\r\nalt3",
    );
    enter(&mut server);
    press(&mut server, Key::Char('g'));
    press(&mut server, Key::Char('k'));
    chord(&mut server, Key::Char('u'), ctrl());
    wheel(&mut server, MouseButton::WheelUp);
    assert_eq!(offset(&server), 0);
    assert_eq!(cursor(&server).row, 0);
    assert!(copy_mode(&server));
    press(&mut server, Key::Char('v'));
    shifted(&mut server, 'G');
    press(&mut server, Key::Char('$'));
    let out = press(&mut server, Key::Char('y'));
    assert_eq!(clipboard(&out), Some(("alt0\nalt1\nalt2\nalt3", false)));
}
