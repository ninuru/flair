use super::*;
use flair_core::{
    command::{PaneRef, WindowRef},
    geom::Axis,
    key::MouseInput,
};

fn split() -> (Server, PaneId, Vec<Effect>) {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(1),
            ClientMsg::Command(Command::Split(Axis::Horizontal)),
        ),
        &mut out,
    );
    let pane = server.client_pane(ClientId(1)).unwrap();
    ack(&mut server, &out);
    (server, pane, Vec::new())
}

fn mouse(
    server: &mut Server,
    client: ClientId,
    point: CellPoint,
    button: MouseButton,
    action: MouseAction,
) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(20),
        Event::Client(
            client,
            ClientMsg::Input(InputEvent::Mouse(MouseInput {
                point,
                button,
                action,
                mods: Mods::default(),
            })),
        ),
        &mut out,
    );
    out
}

#[test]
fn clicking_inactive_pane_selects_it_and_uses_local_selection_coordinates() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"hello".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 0, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let copied = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 4, row: 0 },
        MouseButton::Left,
        MouseAction::Release,
    );
    assert_eq!(server.client_pane(ClientId(1)), Some(PaneId(1)));
    assert_ne!(PaneId(1), second);
    assert!(copied.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::SetClipboard {text,primary:true}) if text=="hello")));
}

#[test]
fn reporting_hover_and_wheel_route_under_pointer_without_changing_focus() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"\x1b[?1003h\x1b[?1006h".to_vec()),
        &mut out,
    );
    for (button, action, expected) in [
        (
            MouseButton::NoButton,
            MouseAction::Motion,
            b"\x1b[<35;3;1M".as_slice(),
        ),
        (
            MouseButton::WheelUp,
            MouseAction::Press,
            b"\x1b[<64;3;1M".as_slice(),
        ),
    ] {
        let out = mouse(
            &mut server,
            ClientId(1),
            CellPoint { col: 2, row: 0 },
            button,
            action,
        );
        assert!(
            out.iter().any(
                |effect| matches!(effect, Effect::WritePty(PaneId(1),bytes) if bytes==expected)
            )
        );
        assert_eq!(server.client_pane(ClientId(1)), Some(second));
    }
}

#[test]
fn separator_drag_resizes_both_panes_and_release_ends_capture() {
    let (mut server, _, _) = split();
    let separator = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry()
        .separators[0]
        .rect
        .origin;
    mouse(
        &mut server,
        ClientId(1),
        separator,
        MouseButton::Left,
        MouseAction::Press,
    );
    let resized = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 1 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 5);
    assert_eq!(server.panes[&PaneId(2)].size.cols, 14);
    assert!(
        resized
            .iter()
            .any(|effect| matches!(effect, Effect::ResizePty(PaneId(1),size) if size.cols==5))
    );
    assert!(
        !resized
            .iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, _)))
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 0 },
        MouseButton::Left,
        MouseAction::Release,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 8, row: 1 },
        MouseButton::NoButton,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 5);
}

fn command(server: &mut Server, client: ClientId, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(client, ClientMsg::Command(command)),
        &mut out,
    );
    out
}

#[test]
fn application_press_selects_target_and_drag_release_stays_with_that_pane() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006h".to_vec()),
        &mut out,
    );
    command(
        &mut server,
        ClientId(1),
        Command::SelectPane(PaneRef::Id(PaneId(1))),
    );
    let origin = server.tree.window(WindowId(1)).unwrap().geometry().panes[1]
        .rect
        .origin;
    let pressed = mouse(
        &mut server,
        ClientId(1),
        CellPoint {
            col: origin.col + 1,
            row: origin.row,
        },
        MouseButton::Left,
        MouseAction::Press,
    );
    assert_eq!(server.client_pane(ClientId(1)), Some(second));
    assert!(pressed.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;2;1M")));
    let motion = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 0, row: 0 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert!(motion.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<32;1;1M")));
    let released = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 0, row: 0 },
        MouseButton::Left,
        MouseAction::Release,
    );
    assert!(released.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;1;1m")));
    assert!(server.clients[&ClientId(1)].mouse.is_none());
}

#[test]
fn selection_drag_crossing_separator_stays_in_original_pane() {
    let (mut server, _, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"leftABCDE".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 0, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 19, row: 0 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    let released = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 19, row: 0 },
        MouseButton::Left,
        MouseAction::Release,
    );
    assert_eq!(server.client_pane(ClientId(1)), Some(PaneId(1)));
    assert!(released.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::SetClipboard {text,primary:true}) if text=="leftABCDE")));
}

#[test]
fn shift_selects_text_in_mouse_reporting_pane() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006hhello".to_vec()),
        &mut out,
    );
    let origin = server.tree.window(WindowId(1)).unwrap().geometry().panes[1]
        .rect
        .origin;
    for (offset, action) in [(0, MouseAction::Press), (4, MouseAction::Release)] {
        server.handle(
            Millis(20),
            Event::Client(
                ClientId(1),
                ClientMsg::Input(InputEvent::Mouse(MouseInput {
                    point: CellPoint {
                        col: origin.col + offset,
                        row: origin.row,
                    },
                    button: MouseButton::Left,
                    action,
                    mods: Mods {
                        shift: true,
                        ..Mods::default()
                    },
                })),
            ),
            &mut out,
        );
    }
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, _)))
    );
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::SetClipboard {text,primary:true}) if text=="hello")));
}

#[test]
fn separator_does_not_forward_mouse_reporting() {
    let (mut server, second, mut out) = split();
    for pane in [PaneId(1), second] {
        server.handle(
            Millis(3),
            Event::PtyOutput(pane, b"\x1b[?1003h\x1b[?1006h".to_vec()),
            &mut out,
        );
    }
    let out = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::WheelUp,
        MouseAction::Press,
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, _)))
    );
    assert_eq!(server.client_pane(ClientId(1)), Some(second));
}

#[test]
fn focus_loss_and_window_switch_cancel_separator_drag() {
    let (mut server, _, _) = split();
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let mut out = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(ClientId(1), ClientMsg::Focus(false)),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 0 },
        MouseButton::NoButton,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 9);
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    command(&mut server, ClientId(1), Command::NewWindow);
    command(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Index(1)),
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 0 },
        MouseButton::NoButton,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 9);
    assert!(server.clients[&ClientId(1)].mouse.is_none());
}

#[test]
fn stacked_separator_drag_uses_full_grid_row_offset() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 20, rows: 10 }),
        ),
        &mut out,
    );
    command(&mut server, ClientId(1), Command::Split(Axis::Vertical));
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 4 },
        MouseButton::Left,
        MouseAction::Press,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 2 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.rows, 2);
    assert_eq!(server.panes[&PaneId(2)].size.rows, 7);
}

#[test]
fn wheel_scrolls_inactive_pane_history_without_selecting_it() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(PaneId(1), b"1\r\n2\r\n3\r\n4\r\n5\r\n6".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 2, row: 0 },
        MouseButton::WheelUp,
        MouseAction::Press,
    );
    assert!(
        server.panes[&PaneId(1)]
            .cached
            .as_ref()
            .unwrap()
            .flags
            .display_offset
            > 0
    );
    assert_eq!(server.client_pane(ClientId(1)), Some(second));
}

#[test]
fn closing_pane_invalidates_separator_capture() {
    let (mut server, _, _) = split();
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    command(&mut server, ClientId(1), Command::ClosePane);
    assert!(server.clients[&ClientId(1)].mouse.is_none());
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 0 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 20);
}

#[test]
fn focus_loss_releases_captured_application_button() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006h".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 11, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let mut lost = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(ClientId(1), ClientMsg::Focus(false)),
        &mut lost,
    );
    assert!(lost.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;2;1m")));
    assert!(server.clients[&ClientId(1)].mouse.is_none());
}

#[test]
fn disconnected_client_releases_captured_button_in_surviving_pane() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006h".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 11, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let mut gone = Vec::new();
    server.handle(Millis(21), Event::ClientGone(ClientId(1)), &mut gone);
    assert!(gone.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;2;1m")));
    assert!(server.panes.contains_key(&second));
}

#[test]
fn clients_in_different_windows_keep_independent_separator_captures() {
    let (mut server, _, mut out) = split();
    server.handle(Millis(3), Event::Client(ClientId(2), hello()), &mut out);
    command(&mut server, ClientId(2), Command::NewWindow);
    command(&mut server, ClientId(2), Command::Split(Axis::Horizontal));
    let second_window = server.clients[&ClientId(2)].view.window;
    mouse(
        &mut server,
        ClientId(2),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 1 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert!(server.clients[&ClientId(2)].mouse.is_some());
    assert_eq!(server.panes[&PaneId(3)].size.cols, 9);
    mouse(
        &mut server,
        ClientId(2),
        CellPoint { col: 7, row: 1 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(3)].size.cols, 7);
    assert_eq!(server.panes[&PaneId(1)].size.cols, 5);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
    assert_eq!(server.clients[&ClientId(2)].view.window, second_window);
}

#[test]
fn window_resize_releases_application_capture_before_resizing_ptys() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006h".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 11, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let mut resized = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 40, rows: 10 }),
        ),
        &mut resized,
    );
    let release = resized.iter().position(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;2;1m")).unwrap();
    let resize = resized
        .iter()
        .position(|effect| matches!(effect, Effect::ResizePty(_, _)))
        .unwrap();
    assert!(release < resize);
    assert!(server.clients[&ClientId(1)].mouse.is_none());
}

#[test]
fn window_resize_ends_separator_drag_before_next_motion() {
    let (mut server, _, _) = split();
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 9, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let mut out = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 41, rows: 10 }),
        ),
        &mut out,
    );
    assert!(server.clients[&ClientId(1)].mouse.is_none());
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 5, row: 0 },
        MouseButton::Left,
        MouseAction::Motion,
    );
    assert_eq!(server.panes[&PaneId(1)].size.cols, 20);
}

#[test]
fn replacing_application_button_capture_releases_previous_button() {
    let (mut server, second, mut out) = split();
    server.handle(
        Millis(3),
        Event::PtyOutput(second, b"\x1b[?1002h\x1b[?1006h".to_vec()),
        &mut out,
    );
    mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 11, row: 0 },
        MouseButton::Left,
        MouseAction::Press,
    );
    let replaced = mouse(
        &mut server,
        ClientId(1),
        CellPoint { col: 11, row: 0 },
        MouseButton::Right,
        MouseAction::Press,
    );
    assert!(replaced.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<0;2;1m")));
    let mut lost = Vec::new();
    server.handle(
        Millis(21),
        Event::Client(ClientId(1), ClientMsg::Focus(false)),
        &mut lost,
    );
    assert!(lost.iter().any(|effect| matches!(effect, Effect::WritePty(pane,bytes) if *pane==second && bytes==b"\x1b[<2;2;1m")));
}
