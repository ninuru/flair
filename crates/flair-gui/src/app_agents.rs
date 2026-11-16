use flair_core::{
    command::{Command, PaneRef, WindowRef},
    geom::Direction,
};

use super::*;
use crate::{chrome_layout::PanelView, panels::AgentsOutcome};

impl App {
    pub(super) fn agents_command(&mut self) {
        let agents = status_parts(&self.status).agents;
        let dirs = self.panels.agents_command(&agents);
        self.explorer_toggled(dirs);
    }

    pub(super) fn agents_key(&mut self, key: &KeyInput) -> bool {
        if !self.agents_visible() {
            return false;
        }
        let config = self
            .preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config);
        let pending = self.status.pending.is_some();
        let agents = status_parts(&self.status).agents;
        match self
            .panels
            .agents_key(key, &config.keys, pending, Instant::now(), &agents)
        {
            Some(outcome) => {
                self.agents_outcome(outcome);
                true
            }
            None => false,
        }
    }

    pub(super) fn agents_text(&self) -> bool {
        self.agents_visible() && self.panels.agents_text()
    }

    pub(super) fn agents_outcome(&mut self, outcome: AgentsOutcome) {
        match outcome {
            AgentsOutcome::Ignored => {}
            AgentsOutcome::Redraw | AgentsOutcome::Blur => self.request_redraw(),
            AgentsOutcome::Jump(window, pane) => {
                self.send(ClientMsg::Command(Command::SelectWindow(WindowRef::Id(
                    window,
                ))));
                self.send(ClientMsg::Command(Command::SelectPane(PaneRef::Id(pane))));
                self.request_redraw();
            }
        }
    }

    pub(super) fn pane_edge(&mut self, direction: Direction) {
        let agents = status_parts(&self.status).agents;
        let side = self.effective().explorer.side;
        if self.side_view_visible() && self.panels.edge(direction, side, &agents) {
            self.request_redraw();
        }
    }

    pub(super) fn leave_panel(&mut self, key: &KeyInput) -> bool {
        if !self.side_view_visible() {
            return false;
        }
        let config = self
            .preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config);
        let pending = self.status.pending.is_some();
        let left = self.panels.leave(
            key,
            &config.keys,
            pending,
            Instant::now(),
            config.explorer.side,
        );
        if left {
            self.send(ClientMsg::CancelPrefix);
            self.request_redraw();
        }
        left
    }

    fn side_view_visible(&self) -> bool {
        self.frame().is_some_and(|frame| {
            frame
                .slots()
                .any(|slot| matches!(slot.view, PanelView::Explorer | PanelView::Agents))
        })
    }

    fn agents_visible(&self) -> bool {
        self.frame()
            .is_some_and(|frame| frame.slots().any(|slot| slot.view == PanelView::Agents))
    }
}
