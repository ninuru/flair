use super::*;

fn at(row: u16) -> CellPoint {
    CellPoint { col: 3, row }
}

#[test]
fn status_row_press_selects_and_other_status_events_drop() {
    assert_eq!(
        route_mouse(at(0), MouseButton::Left, MouseAction::Press, false),
        MouseRoute::Status
    );
    for (button, action) in [
        (MouseButton::Left, MouseAction::Release),
        (MouseButton::NoButton, MouseAction::Motion),
        (MouseButton::Right, MouseAction::Press),
        (MouseButton::Middle, MouseAction::Press),
        (MouseButton::WheelUp, MouseAction::Press),
    ] {
        assert_eq!(route_mouse(at(0), button, action, false), MouseRoute::Drop);
    }
}

#[test]
fn pane_drag_keeps_reaching_the_server_over_the_status_row() {
    let mut held = false;
    let mut routes = Vec::new();
    for (row, button, action) in [
        (3, MouseButton::Left, MouseAction::Press),
        (0, MouseButton::Left, MouseAction::Motion),
        (0, MouseButton::Left, MouseAction::Release),
        (0, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(at(row), button, action, held);
        held = held_after(held, button, action, route);
        routes.push(route);
    }
    assert_eq!(
        routes,
        [
            MouseRoute::Server,
            MouseRoute::Server,
            MouseRoute::Server,
            MouseRoute::Drop
        ]
    );
    assert!(!held);
}

#[test]
fn wheel_and_status_presses_do_not_start_a_capture() {
    assert!(!held_after(
        false,
        MouseButton::WheelUp,
        MouseAction::Press,
        MouseRoute::Server
    ));
    assert!(!held_after(
        false,
        MouseButton::Left,
        MouseAction::Press,
        MouseRoute::Status
    ));
    assert!(held_after(
        false,
        MouseButton::Right,
        MouseAction::Press,
        MouseRoute::Server
    ));
}
