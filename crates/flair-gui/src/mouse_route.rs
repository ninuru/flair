use flair_core::{
    command::{Command, WindowRef},
    geom::{CellPoint, CellRect, GridSize},
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
    Panel,
    Overlay,
}

pub(crate) fn route_mouse(
    in_chrome: bool,
    button: MouseButton,
    action: MouseAction,
    capture: Capture,
) -> MouseRoute {
    match capture {
        Capture::Server => MouseRoute::Server,
        Capture::Bar | Capture::Panel | Capture::Overlay => MouseRoute::Drop,
        Capture::Idle if !in_chrome => MouseRoute::Server,
        Capture::Idle if button == MouseButton::Left && action == MouseAction::Press => {
            MouseRoute::Bar
        }
        Capture::Idle => MouseRoute::Drop,
    }
}

pub(crate) fn overlay_takes_mouse(open: bool, capture: Capture) -> bool {
    match capture {
        Capture::Panel => false,
        Capture::Overlay => true,
        Capture::Idle | Capture::Server | Capture::Bar => open,
    }
}

pub(crate) fn bar_command(target: HitTarget) -> Option<Command> {
    match target {
        HitTarget::Window(window) | HitTarget::Tab(window) => {
            Some(Command::SelectWindow(WindowRef::Id(window)))
        }
        HitTarget::Session => Some(Command::ChooseSession),
        HitTarget::NewTab => Some(Command::NewWindow),
        HitTarget::CloseTab(window) => Some(Command::KillWindow(window)),
        HitTarget::Prompt { .. }
        | HitTarget::Edge(_)
        | HitTarget::Switch(_)
        | HitTarget::PanelBody
        | HitTarget::Row(_)
        | HitTarget::Agent(_)
        | HitTarget::AgentsButton => None,
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
        | (
            MouseAction::Press,
            Capture::Server | Capture::Bar | Capture::Panel | Capture::Overlay,
            _,
        ) => capture,
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
    pane_row(layout, active, point).map(|(pane, _)| pane)
}

pub(crate) fn pane_row(
    layout: Option<&WindowLayout>,
    active: Option<PaneId>,
    point: CellPoint,
) -> Option<(PaneId, u16)> {
    match layout {
        Some(layout) => layout
            .panes
            .iter()
            .find(|placement| placement.rect.contains(point))
            .map(|placement| (placement.pane, point.row - placement.rect.origin.row)),
        None => active.map(|pane| (pane, point.row)),
    }
}

pub(crate) fn pane_cell(
    layout: Option<&WindowLayout>,
    active: Option<PaneId>,
    size: GridSize,
    point: CellPoint,
) -> Option<(PaneId, CellPoint, CellRect)> {
    match layout {
        Some(layout) => layout
            .panes
            .iter()
            .find(|placement| placement.rect.contains(point))
            .map(|placement| {
                let local = CellPoint {
                    col: point.col - placement.rect.origin.col,
                    row: point.row - placement.rect.origin.row,
                };
                (placement.pane, local, placement.rect)
            }),
        None => active.map(|pane| {
            let rect = CellRect {
                origin: CellPoint { col: 0, row: 0 },
                size,
            };
            (pane, point, rect)
        }),
    }
}

#[cfg(test)]
#[path = "mouse_route_tests.rs"]
mod tests;
