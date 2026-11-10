use flair_core::{
    command::{Command, WindowRef},
    geom::CellPoint,
    ids::PaneId,
    key::{MouseAction, MouseButton},
    proto::WindowLayout,
};

use crate::chrome_layout::HitTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseRoute {
    Bar,
    Server,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capture {
    Idle,
    Server,
    Bar,
    Overlay,
}

pub(crate) fn route_mouse(
    in_bar: bool,
    button: MouseButton,
    action: MouseAction,
    capture: Capture,
) -> MouseRoute {
    match capture {
        Capture::Server => MouseRoute::Server,
        Capture::Bar | Capture::Overlay => MouseRoute::Drop,
        Capture::Idle if !in_bar => MouseRoute::Server,
        Capture::Idle if button == MouseButton::Left && action == MouseAction::Press => {
            MouseRoute::Bar
        }
        Capture::Idle => MouseRoute::Drop,
    }
}

pub(crate) fn bar_command(target: HitTarget) -> Command {
    match target {
        HitTarget::Window(window) => Command::SelectWindow(WindowRef::Id(window)),
        HitTarget::Session => Command::ChooseSession,
    }
}

pub(crate) fn capture_after(
    capture: Capture,
    button: MouseButton,
    action: MouseAction,
    route: MouseRoute,
) -> Capture {
    match (action, capture, route) {
        (MouseAction::Release, _, _) => Capture::Idle,
        (MouseAction::Motion, _, _)
        | (MouseAction::Press, Capture::Server | Capture::Bar | Capture::Overlay, _) => capture,
        (MouseAction::Press, Capture::Idle, MouseRoute::Bar) => Capture::Bar,
        (MouseAction::Press, Capture::Idle, MouseRoute::Server)
            if matches!(
                button,
                MouseButton::Left | MouseButton::Middle | MouseButton::Right
            ) =>
        {
            Capture::Server
        }
        (MouseAction::Press, Capture::Idle, MouseRoute::Server | MouseRoute::Drop) => Capture::Idle,
    }
}

pub(crate) fn paste_target(
    layout: Option<&WindowLayout>,
    active: Option<PaneId>,
    point: CellPoint,
) -> Option<PaneId> {
    match layout {
        Some(layout) => layout
            .panes
            .iter()
            .find(|placement| placement.rect.contains(point))
            .map(|placement| placement.pane),
        None => active,
    }
}

#[cfg(test)]
#[path = "mouse_route_tests.rs"]
mod tests;
