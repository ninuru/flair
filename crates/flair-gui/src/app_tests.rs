use super::*;

#[test]
fn bar_press_goes_to_the_bar_and_other_bar_events_drop() {
    assert_eq!(
        route_mouse(true, MouseButton::Left, MouseAction::Press, false),
        MouseRoute::Bar
    );
    for (button, action) in [
        (MouseButton::Left, MouseAction::Release),
        (MouseButton::NoButton, MouseAction::Motion),
        (MouseButton::Right, MouseAction::Press),
        (MouseButton::Middle, MouseAction::Press),
        (MouseButton::WheelUp, MouseAction::Press),
    ] {
        assert_eq!(route_mouse(true, button, action, false), MouseRoute::Drop);
    }
}

#[test]
fn route_mouse_table() {
    for (in_bar, button, action, held, route) in [
        (
            false,
            MouseButton::Left,
            MouseAction::Press,
            false,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::NoButton,
            MouseAction::Motion,
            false,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::WheelDown,
            MouseAction::Press,
            false,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Press,
            false,
            MouseRoute::Bar,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Motion,
            true,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Release,
            true,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::WheelUp,
            MouseAction::Press,
            false,
            MouseRoute::Drop,
        ),
    ] {
        assert_eq!(
            route_mouse(in_bar, button, action, held),
            route,
            "{in_bar} {button:?} {action:?} {held}"
        );
    }
}

#[test]
fn pane_drag_keeps_reaching_the_server_over_the_bar() {
    let mut held = false;
    let mut routes = Vec::new();
    for (in_bar, button, action) in [
        (false, MouseButton::Left, MouseAction::Press),
        (true, MouseButton::Left, MouseAction::Motion),
        (true, MouseButton::Left, MouseAction::Release),
        (true, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(in_bar, button, action, held);
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
fn wheel_and_bar_presses_do_not_start_a_capture() {
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
        MouseRoute::Bar
    ));
    assert!(held_after(
        false,
        MouseButton::Right,
        MouseAction::Press,
        MouseRoute::Server
    ));
}

#[test]
fn bar_hits_map_to_commands() {
    assert_eq!(
        bar_command(HitTarget::Window(flair_core::ids::WindowId(4))),
        Command::SelectWindow(WindowRef::Id(flair_core::ids::WindowId(4)))
    );
    assert_eq!(bar_command(HitTarget::Session), Command::ChooseSession);
}
