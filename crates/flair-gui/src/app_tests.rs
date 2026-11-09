use super::*;

#[test]
fn bar_press_goes_to_the_bar_and_other_bar_events_drop() {
    assert_eq!(
        route_mouse(true, MouseButton::Left, MouseAction::Press, Capture::Idle),
        MouseRoute::Bar
    );
    for (button, action) in [
        (MouseButton::Left, MouseAction::Release),
        (MouseButton::NoButton, MouseAction::Motion),
        (MouseButton::Right, MouseAction::Press),
        (MouseButton::Middle, MouseAction::Press),
        (MouseButton::WheelUp, MouseAction::Press),
    ] {
        assert_eq!(
            route_mouse(true, button, action, Capture::Idle),
            MouseRoute::Drop
        );
    }
}

#[test]
fn an_overlay_gesture_is_dropped_until_release() {
    assert_eq!(
        route_mouse(
            false,
            MouseButton::Left,
            MouseAction::Motion,
            Capture::Overlay
        ),
        MouseRoute::Drop
    );
    assert_eq!(
        route_mouse(
            false,
            MouseButton::Left,
            MouseAction::Release,
            Capture::Overlay
        ),
        MouseRoute::Drop
    );
    assert_eq!(
        capture_after(
            Capture::Overlay,
            MouseButton::Left,
            MouseAction::Motion,
            MouseRoute::Drop
        ),
        Capture::Overlay
    );
    assert_eq!(
        capture_after(
            Capture::Overlay,
            MouseButton::Left,
            MouseAction::Release,
            MouseRoute::Drop
        ),
        Capture::Idle
    );
}

#[test]
fn route_mouse_table() {
    for (in_bar, button, action, held, route) in [
        (
            false,
            MouseButton::Left,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::NoButton,
            MouseAction::Motion,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::WheelDown,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Bar,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Motion,
            Capture::Server,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Release,
            Capture::Server,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::WheelUp,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Drop,
        ),
    ] {
        assert_eq!(
            route_mouse(in_bar, button, action, held),
            route,
            "{in_bar} {button:?} {action:?} {held:?}"
        );
    }
}

#[test]
fn pane_drag_keeps_reaching_the_server_over_the_bar() {
    let mut capture = Capture::Idle;
    let mut routes = Vec::new();
    for (in_bar, button, action) in [
        (false, MouseButton::Left, MouseAction::Press),
        (true, MouseButton::Left, MouseAction::Motion),
        (true, MouseButton::Left, MouseAction::Release),
        (true, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(in_bar, button, action, capture);
        capture = capture_after(capture, button, action, route);
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
    assert_eq!(capture, Capture::Idle);
}

#[test]
fn tab_drag_into_a_pane_never_reaches_the_server() {
    let mut capture = Capture::Idle;
    let mut routes = Vec::new();
    for (in_bar, button, action) in [
        (true, MouseButton::Left, MouseAction::Press),
        (true, MouseButton::Left, MouseAction::Motion),
        (false, MouseButton::Left, MouseAction::Motion),
        (false, MouseButton::Left, MouseAction::Release),
        (false, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(in_bar, button, action, capture);
        capture = capture_after(capture, button, action, route);
        routes.push(route);
    }
    assert_eq!(
        routes,
        [
            MouseRoute::Bar,
            MouseRoute::Drop,
            MouseRoute::Drop,
            MouseRoute::Drop,
            MouseRoute::Server
        ]
    );
    assert_eq!(capture, Capture::Idle);
}

#[test]
fn wheel_presses_do_not_start_a_capture_and_bar_presses_capture_the_bar() {
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::WheelUp,
            MouseAction::Press,
            MouseRoute::Server
        ),
        Capture::Idle
    );
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::Left,
            MouseAction::Press,
            MouseRoute::Bar
        ),
        Capture::Bar
    );
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::Right,
            MouseAction::Press,
            MouseRoute::Server
        ),
        Capture::Server
    );
}

#[test]
fn bar_hits_map_to_commands() {
    assert_eq!(
        bar_command(HitTarget::Window(flair_core::ids::WindowId(4))),
        Command::SelectWindow(WindowRef::Id(flair_core::ids::WindowId(4)))
    );
    assert_eq!(bar_command(HitTarget::Session), Command::ChooseSession);
}
