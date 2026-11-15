use flair_core::command::{Command, PaneRef, WindowRef};

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

    fn agents_visible(&self) -> bool {
        self.frame()
            .is_some_and(|frame| frame.slots().any(|slot| slot.view == PanelView::Agents))
    }
}
