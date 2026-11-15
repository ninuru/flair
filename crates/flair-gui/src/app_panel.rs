use flair_core::{command::Command, settings::Value};
use winit::window::CursorIcon;

use super::*;
use crate::{
    chrome_layout::{Frame, PanelView, max_width},
    overlay_layout::inside,
    panels::Drag,
};

impl App {
    pub(super) fn padding(&self, scale: f64) -> (f32, f32) {
        chrome_layout::padding(&self.effective().window, scale, self.zen.on())
    }

    pub(super) fn frame(&self) -> Option<Frame> {
        let surface = self.surface.as_ref()?;
        let px = surface.window.inner_size();
        let scale = surface.window.scale_factor();
        Some(chrome_layout::frame(
            (px.width, px.height),
            scale as f32,
            self.zen.on(),
            self.padding(scale),
            surface.fonts.metrics(),
            self.panels.dock(self.effective(), self.zen.on()),
        ))
    }

    pub(super) fn size(&self) -> GridSize {
        match (&self.surface, self.frame()) {
            (Some(surface), Some(frame)) => chrome_layout::grid_size(
                &frame,
                self.padding(surface.window.scale_factor()),
                surface.fonts.metrics(),
            ),
            (Some(_) | None, _) => GridSize { cols: 80, rows: 24 },
        }
    }

    pub(super) fn cell(&self) -> Option<CellPoint> {
        let surface = self.surface.as_ref()?;
        let frame = self.frame()?;
        let metrics = surface.fonts.metrics();
        let padding = self.padding(surface.window.scale_factor());
        Some(mouse::to_cell(
            (self.pointer.x, self.pointer.y),
            &metrics,
            chrome_layout::grid_origin(&frame, padding, metrics),
            chrome_layout::grid_size(&frame, padding, metrics),
        ))
    }

    pub(super) fn in_chrome(&self) -> bool {
        let Some(surface) = &self.surface else {
            return false;
        };
        let bar = !self.zen.on()
            && self.pointer.y
                < f64::from(chrome_layout::bar_height(
                    surface.window.scale_factor() as f32
                ));
        bar || self.panel_target().is_some()
    }

    fn panel_target(&self) -> Option<HitTarget> {
        let target = chrome_layout::hit_test(&self.hits, (self.pointer.x, self.pointer.y))?;
        match target {
            HitTarget::Tab(_)
            | HitTarget::NewTab
            | HitTarget::CloseTab(_)
            | HitTarget::Edge(_)
            | HitTarget::Switch(_)
            | HitTarget::PanelBody
            | HitTarget::Row(_) => Some(target),
            HitTarget::Window(_)
            | HitTarget::Session
            | HitTarget::Prompt { .. }
            | HitTarget::ExplorerButton => None,
        }
    }

    fn scale(&self) -> f32 {
        self.surface
            .as_ref()
            .map_or(1.0, |surface| surface.window.scale_factor() as f32)
    }

    pub(super) fn panel_mouse(&mut self, button: MouseButton, action: MouseAction) -> bool {
        match (self.capture, action) {
            (Capture::Panel, MouseAction::Motion) => {
                self.panel_drag();
                true
            }
            (Capture::Panel, MouseAction::Release) => {
                self.capture = Capture::Idle;
                self.drop_drag();
                true
            }
            (Capture::Panel, MouseAction::Press) => true,
            (Capture::Idle, action) => {
                let Some(target) = self.panel_target() else {
                    return false;
                };
                if action == MouseAction::Press
                    && matches!(
                        button,
                        MouseButton::Left | MouseButton::Middle | MouseButton::Right
                    )
                {
                    self.capture = Capture::Panel;
                    self.panel_press(button, target);
                }
                true
            }
            (Capture::Server | Capture::Bar | Capture::Overlay, _) => false,
        }
    }

    fn panel_press(&mut self, button: MouseButton, target: HitTarget) {
        let point = (self.pointer.x as f32, self.pointer.y as f32);
        let left = button == MouseButton::Left;
        let middle = button == MouseButton::Middle;
        let command = match target {
            HitTarget::Tab(window) if left => {
                self.panels.drag = Some(Drag::Tab {
                    window,
                    from: point.1,
                    moved: false,
                });
                bar_command(target)
            }
            HitTarget::Tab(window) | HitTarget::CloseTab(window) if middle => {
                Some(Command::KillWindow(window))
            }
            HitTarget::Edge(side) if left => {
                let scale = self.scale();
                self.panels.drag =
                    self.frame()
                        .and_then(|frame| frame.slot(side))
                        .map(|slot| Drag::Edge {
                            side,
                            from: point.0,
                            width: (slot.rect.w / scale).round(),
                        });
                None
            }
            HitTarget::Switch(view) if left => {
                self.panels.shown = view;
                self.panels.focused = self.panels.open && view == PanelView::Explorer;
                None
            }
            HitTarget::Row(row) if left => {
                let outcome = self.panels.click(row, Instant::now());
                self.explorer_outcome(outcome);
                None
            }
            HitTarget::NewTab | HitTarget::CloseTab(_) if left => bar_command(target),
            HitTarget::Tab(_)
            | HitTarget::NewTab
            | HitTarget::CloseTab(_)
            | HitTarget::Edge(_)
            | HitTarget::Switch(_)
            | HitTarget::PanelBody
            | HitTarget::Row(_)
            | HitTarget::ExplorerButton
            | HitTarget::Window(_)
            | HitTarget::Session
            | HitTarget::Prompt { .. } => None,
        };
        if let Some(command) = command {
            self.send(ClientMsg::Command(command));
        }
        self.request_redraw();
    }

    fn panel_drag(&mut self) {
        let point = (self.pointer.x as f32, self.pointer.y as f32);
        let scale = self.scale();
        match self.panels.drag {
            Some(Drag::Tab { .. }) => {
                self.panels.drag_tab(point.1, scale);
                self.request_redraw();
            }
            Some(Drag::Edge { side, .. }) => {
                let (Some(surface), Some(frame)) = (&self.surface, self.frame()) else {
                    return;
                };
                let Some(slot) = frame.slot(side) else {
                    return;
                };
                let view = match slot.shared {
                    true => PanelView::Explorer,
                    false => slot.view,
                };
                let padding = self.padding(surface.window.scale_factor());
                let limit = max_width(&frame, side, padding, surface.fonts.metrics(), scale);
                let before = self.size();
                if self.panels.drag_edge(view, point.0, limit, scale) {
                    let after = self.size();
                    if after != before {
                        self.send(ClientMsg::Resize(after));
                    }
                    self.request_redraw();
                }
            }
            None => {}
        }
    }

    fn drop_drag(&mut self) {
        let drag = self.panels.drag.take();
        match drag {
            Some(Drag::Tab {
                window,
                moved: true,
                ..
            }) => {
                if let Some(frame) = self
                    .frame()
                    .filter(|frame| frame.slots().any(|slot| slot.view == PanelView::Tabs))
                {
                    let index = self.panels.drop_index(
                        &frame,
                        self.pointer.y as f32,
                        self.status.windows.len(),
                        self.scale(),
                    );
                    self.send(ClientMsg::Command(Command::MoveWindow { window, index }));
                }
            }
            Some(Drag::Tab { moved: false, .. }) | None => {}
            Some(Drag::Edge { .. }) => {
                if let Some((view, width)) = self.panels.live_width {
                    let key = match view {
                        PanelView::Tabs => "tabs.width",
                        PanelView::Explorer => "explorer.width",
                    };
                    self.save_setting(key, Some(Value::Integer(width as i64)));
                }
                let before = self.size();
                self.panels.live_width = None;
                let after = self.size();
                if after != before {
                    self.send(ClientMsg::Resize(after));
                }
            }
        }
        self.update_cursor();
        self.request_redraw();
    }

    pub(super) fn panel_wheel(&mut self, lines: i32) -> bool {
        if self.panel_target().is_none() {
            return false;
        }
        let point = (self.pointer.x as f32, self.pointer.y as f32);
        let view = self.frame().and_then(|frame| {
            frame
                .slots()
                .find(|slot| inside(slot.rect, point))
                .map(|slot| slot.view)
        });
        match view {
            Some(PanelView::Explorer) => self.panels.scroll_explorer(-lines),
            Some(PanelView::Tabs) | None => {
                self.panels.scroll_tabs(-lines, self.status.windows.len());
            }
        }
        self.request_redraw();
        true
    }

    pub(super) fn update_cursor(&mut self) {
        let edge = matches!(self.panels.drag, Some(Drag::Edge { .. }))
            || (!self.overlays.is_open()
                && matches!(self.panel_target(), Some(HitTarget::Edge(_))));
        if edge == self.resize_cursor {
            return;
        }
        self.resize_cursor = edge;
        if let Some(surface) = &self.surface {
            surface.window.set_cursor(match edge {
                true => CursorIcon::EwResize,
                false => CursorIcon::Default,
            });
        }
    }
}
