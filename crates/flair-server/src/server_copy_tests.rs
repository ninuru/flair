use super::*;
use flair_core::{
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
        size: GridSize { cols: 20, rows: 5 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target: Target::Main,
    }
}

fn setup() -> Server {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
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
    mouse(server, 1, button, MouseAction::Press)
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
        input(
            Key::Named(NamedKey::Enter),
            Mods::default(),
            KeyAction::Press,
        ),
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
    let out = mouse(&mut server, 4, MouseButton::Left, MouseAction::Press);
    assert!(!copy_mode(&server));
    assert!(!status(&out).unwrap().copy);
    assert_eq!(offset(&server), 0);
    send(
        &mut server,
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 4, row: 4 },
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
        mouse(&mut server, 1, button, MouseAction::Press);
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
