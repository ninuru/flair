use std::time::Instant;

use flair_core::{
    command::{Command, PaneRef, WindowRef},
    key::KeyInput,
    proto::ClientMsg,
};
use flair_gui::{
    chrome_layout::PanelView,
    panels::AgentsOutcome,
    status::{AgentParts, status_parts},
};

use super::Capture;

impl Capture {
    pub(super) fn agents_command(&mut self) -> Result<Option<ClientMsg>, String> {
        let dirs = self.panels.agents_command(&self.agents());
        self.load(dirs);
        self.refit()
    }

    pub(super) fn agents_key(&mut self, key: &KeyInput) -> Option<Vec<ClientMsg>> {
        if !self.agents_visible() {
            return None;
        }
        let config = self
            .preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config);
        let pending = self.status.pending.is_some();
        let agents = status_parts(&self.status).agents;
        let outcome =
            self.panels
                .agents_key(key, &config.keys, pending, Instant::now(), &agents)?;
        Some(match outcome {
            AgentsOutcome::Ignored | AgentsOutcome::Redraw | AgentsOutcome::Blur => Vec::new(),
            AgentsOutcome::Jump(window, pane) => vec![
                ClientMsg::Command(Command::SelectWindow(WindowRef::Id(window))),
                ClientMsg::Command(Command::SelectPane(PaneRef::Id(pane))),
            ],
        })
    }

    pub(super) fn agents_text(&self) -> bool {
        self.agents_visible() && self.panels.agents_text()
    }

    pub(super) fn agents_lines(&self) -> Option<String> {
        if !self.agents_visible() {
            return None;
        }
        let agents = self.agents();
        let mut lines = vec!["[agents]".to_owned()];
        if agents.is_empty() {
            lines.push("no agents".to_owned());
        }
        let mut session = None;
        for agent in &agents {
            if session != Some(agent.session.as_str()) {
                session = Some(agent.session.as_str());
                lines.push(agent.session.clone());
            }
            let unseen = match agent.unseen {
                true => " (unseen)",
                false => "",
            };
            lines.push(format!("  {} - {}{unseen}", agent.title, agent.label));
        }
        Some(lines.join("\n"))
    }

    fn agents(&self) -> Vec<AgentParts> {
        status_parts(&self.status).agents
    }

    fn agents_visible(&self) -> bool {
        match &self.pixels {
            Some(pixels) => pixels
                .frame(self.effective(), self.zen, &self.panels)
                .slots()
                .any(|slot| slot.view == PanelView::Agents),
            None => self.panels.open && !self.zen && self.panels.shown == PanelView::Agents,
        }
    }
}
