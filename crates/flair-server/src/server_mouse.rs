use super::*;
use crate::client::MouseCapture;
use flair_core::{
    command::PaneRef,
    geom::CellPoint,
    key::{MouseAction, MouseButton, MouseInput},
    proto::PanePlacement,
};
use flair_term::{encode::encode_mouse, terminal::MouseMode};

impl Server {
    pub(super) fn cancel_mouse(&mut self, id: ClientId, out: &mut Vec<Effect>) {
        let client = self.clients.get_mut(&id).unwrap();
        client.selecting = false;
        if let Some(MouseCapture::Reporting { pane, input }) = client.mouse.take()
            && let Some(pane_state) = self.panes.get(&pane)
            && let Some(bytes) = encode_mouse(
                &MouseInput {
                    action: MouseAction::Release,
                    ..input
                },
                &pane_state.terminal.modes(),
            )
        {
            out.push(Effect::WritePty(pane, bytes));
        }
    }

    pub(super) fn mouse_input(
        &mut self,
        now: Millis,
        id: ClientId,
        mut mouse: MouseInput,
        out: &mut Vec<Effect>,
    ) {
        if matches!(mouse.action, MouseAction::Motion | MouseAction::Release)
            && let Some(capture) = self.clients[&id].mouse.clone()
        {
            self.captured_mouse(id, mouse, capture, out);
            return;
        }
        let window_id = self.clients[&id].view.window;
        if mouse.action == MouseAction::Press && is_button(mouse.button) {
            self.cancel_mouse(id, out);
        }
        let window = self.tree.window(window_id).unwrap();
        if mouse.button == MouseButton::Left
            && mouse.action == MouseAction::Press
            && !window.zoomed
            && let Some(path) = window.layout.separator_at(window.area(), mouse.point)
        {
            let client = self.clients.get_mut(&id).unwrap();
            client.mouse = Some(MouseCapture::Separator {
                window: window_id,
                path,
            });
            client.selecting = false;
            client.last_click = None;
            return;
        }
        let Some(placement) = window
            .geometry(self.band())
            .panes
            .into_iter()
            .find(|placement| placement.rect.contains(mouse.point))
        else {
            return;
        };
        let pane = placement.pane;
        if mouse.action == MouseAction::Press
            && is_button(mouse.button)
            && window.active_pane != pane
        {
            self.select_pane(id, PaneRef::Id(pane), out);
        }
        mouse.point = local_point(mouse.point, placement);
        let copy = self.panes[&pane].terminal.copy_mode();
        let wheel = matches!(mouse.button, MouseButton::WheelUp | MouseButton::WheelDown);
        if copy && mouse.action == MouseAction::Press && is_button(mouse.button) {
            self.exit_copy_mode(pane);
        }
        let modes = self.panes[&pane].terminal.modes();
        if modes.mouse != MouseMode::Off && !mouse.mods.shift && !(copy && wheel) {
            if mouse.action == MouseAction::Press && is_button(mouse.button) {
                self.clients.get_mut(&id).unwrap().mouse =
                    Some(MouseCapture::Reporting { pane, input: mouse });
            }
            if let Some(bytes) = encode_mouse(&mouse, &modes) {
                out.push(Effect::WritePty(pane, bytes));
            }
            return;
        }
        match (mouse.button, mouse.action) {
            (MouseButton::Left, MouseAction::Press) => {
                let client = self.clients.get_mut(&id).unwrap();
                let clicks = client
                    .last_click
                    .filter(|(last, point, _)| {
                        now.saturating_sub(*last).0 <= 400 && *point == mouse.point
                    })
                    .map_or(1, |(_, _, clicks)| clicks % 3 + 1);
                client.last_click = Some((now, mouse.point, clicks));
                client.selecting = true;
                client.mouse = Some(MouseCapture::Selection(pane));
                let pane = self.panes.get_mut(&pane).unwrap();
                pane.terminal.select_start(mouse.point, clicks);
                pane.dirty = true;
            }
            (MouseButton::WheelUp | MouseButton::WheelDown, MouseAction::Press) => {
                self.copy_wheel(pane, mouse.button == MouseButton::WheelUp);
            }
            (MouseButton::Left, MouseAction::Motion | MouseAction::Release)
            | (
                MouseButton::Middle
                | MouseButton::Right
                | MouseButton::NoButton
                | MouseButton::WheelLeft
                | MouseButton::WheelRight,
                _,
            )
            | (
                MouseButton::WheelUp | MouseButton::WheelDown,
                MouseAction::Motion | MouseAction::Release,
            ) => {}
        }
    }

    fn captured_mouse(
        &mut self,
        id: ClientId,
        mut mouse: MouseInput,
        capture: MouseCapture,
        out: &mut Vec<Effect>,
    ) {
        if let MouseCapture::Separator { window, path } = capture {
            if self.clients[&id].view.window != window
                || self.tree.window(window).is_none_or(|window| window.zoomed)
            {
                self.clients.get_mut(&id).unwrap().mouse = None;
                return;
            }
            if mouse.action == MouseAction::Motion || mouse.button == MouseButton::Left {
                let layout = self.tree.window_mut(window).unwrap();
                let area = layout.area();
                if layout.layout.resize_separator(&path, area, mouse.point) {
                    self.invalidate_window(window, out);
                    self.clients.get_mut(&id).unwrap().mouse =
                        Some(MouseCapture::Separator { window, path });
                }
            }
            if mouse.action == MouseAction::Release && mouse.button == MouseButton::Left {
                self.clients.get_mut(&id).unwrap().mouse = None;
            }
            return;
        }
        let pane = match capture {
            MouseCapture::Selection(pane) | MouseCapture::Reporting { pane, .. } => pane,
            MouseCapture::Separator { .. } => unreachable!(),
        };
        let Some(placement) = self
            .tree
            .window(self.clients[&id].view.window)
            .unwrap()
            .geometry(self.band())
            .panes
            .into_iter()
            .find(|placement| placement.pane == pane)
        else {
            let client = self.clients.get_mut(&id).unwrap();
            client.mouse = None;
            client.selecting = false;
            return;
        };
        mouse.point = local_point(mouse.point, placement);
        match capture {
            MouseCapture::Selection(pane) => {
                if mouse.action == MouseAction::Motion || mouse.button == MouseButton::Left {
                    let state = self.panes.get_mut(&pane).unwrap();
                    state.terminal.select_update(mouse.point);
                    state.dirty = true;
                }
                if mouse.action == MouseAction::Release && mouse.button == MouseButton::Left {
                    let client = self.clients.get_mut(&id).unwrap();
                    client.mouse = None;
                    client.selecting = false;
                    if let Some(text) = self.panes[&pane].terminal.selection_text() {
                        out.push(Effect::Send(
                            id,
                            ServerMsg::SetClipboard {
                                text,
                                primary: true,
                            },
                        ));
                    }
                }
            }
            MouseCapture::Reporting { pane, mut input } => {
                let button = input.button;
                if mouse.action == MouseAction::Motion {
                    mouse.button = button;
                }
                let modes = self.panes[&pane].terminal.modes();
                if let Some(bytes) = encode_mouse(&mouse, &modes) {
                    out.push(Effect::WritePty(pane, bytes));
                }
                if mouse.action == MouseAction::Release && mouse.button == button {
                    self.clients.get_mut(&id).unwrap().mouse = None;
                } else {
                    input.point = mouse.point;
                    input.mods = mouse.mods;
                    self.clients.get_mut(&id).unwrap().mouse =
                        Some(MouseCapture::Reporting { pane, input });
                }
            }
            MouseCapture::Separator { .. } => unreachable!(),
        }
    }
}

fn local_point(point: CellPoint, placement: PanePlacement) -> CellPoint {
    CellPoint {
        col: point
            .col
            .saturating_sub(placement.rect.origin.col)
            .min(placement.rect.size.cols.saturating_sub(1)),
        row: point
            .row
            .saturating_sub(placement.rect.origin.row)
            .min(placement.rect.size.rows.saturating_sub(1)),
    }
}

fn is_button(button: MouseButton) -> bool {
    match button {
        MouseButton::Left | MouseButton::Middle | MouseButton::Right => true,
        MouseButton::WheelUp
        | MouseButton::WheelDown
        | MouseButton::WheelLeft
        | MouseButton::WheelRight
        | MouseButton::NoButton => false,
    }
}
