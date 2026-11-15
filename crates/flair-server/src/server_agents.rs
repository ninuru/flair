use super::*;
use flair_core::{
    agent::{AgentKind, AgentRecord, AgentReport, AgentState},
    status::AgentRow,
    tree::{Session, Window},
};

const AGENT_LIMIT: usize = 64;
const NOTIFY_INTERVAL: Millis = Millis(3000);

type Summary = Option<(AgentState, Option<AgentKind>)>;

pub(super) fn apply(records: &mut BTreeMap<String, AgentRecord>, report: AgentReport) {
    match report {
        AgentReport::Set { id, mut record } => {
            if !records.contains_key(&id) && records.len() >= AGENT_LIMIT {
                log::debug!("ignoring agent record {id:?}: {AGENT_LIMIT} records already");
                return;
            }
            if record.app.is_none() {
                record.app = ancestors(&id).find_map(|parent| records.get(parent)?.app.clone());
            }
            records.insert(id, record);
        }
        AgentReport::Clear(Some(id)) => records.retain(|key, _| !in_subtree(key, &id)),
        AgentReport::Clear(None) => records.clear(),
    }
}

pub(super) fn summary(records: &BTreeMap<String, AgentRecord>) -> Option<(&String, &AgentRecord)> {
    records
        .iter()
        .rev()
        .max_by_key(|(_, record)| record.state.rank())
}

fn summary_key(records: &BTreeMap<String, AgentRecord>) -> Summary {
    summary(records).map(|(_, record)| (record.state, record.kind))
}

fn ancestors(id: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(id).filter(|id| !id.is_empty());
    std::iter::from_fn(move || {
        let parent = rest?.rsplit_once('/').map_or("", |(parent, _)| parent);
        rest = Some(parent).filter(|parent| !parent.is_empty());
        Some(parent)
    })
}

fn in_subtree(key: &str, id: &str) -> bool {
    key.strip_prefix(id)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

fn lasting(state: AgentState) -> bool {
    match state {
        AgentState::Done | AgentState::Error => true,
        AgentState::Idle | AgentState::Working | AgentState::Blocked => false,
    }
}

fn alerting(state: AgentState) -> bool {
    match state {
        AgentState::Blocked | AgentState::Done | AgentState::Error => true,
        AgentState::Idle | AgentState::Working => false,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Server {
    pub(super) fn agent_report(
        &mut self,
        now: Millis,
        pane: PaneId,
        report: AgentReport,
        out: &mut Vec<Effect>,
    ) {
        let Some(state) = self.panes.get_mut(&pane) else {
            return;
        };
        let before = summary_key(&state.agents);
        apply(&mut state.agents, report);
        if self.agents_changed(pane, before) {
            self.notify(now, pane, out);
        }
    }

    pub(super) fn drop_agents(&mut self, pane: PaneId) {
        let Some(state) = self.panes.get_mut(&pane) else {
            return;
        };
        if state.agents.is_empty() {
            return;
        }
        let before = summary_key(&state.agents);
        let unseen = state.unseen;
        state
            .agents
            .retain(|_, record| unseen && lasting(record.state));
        self.agents_changed(pane, before);
    }

    fn agents_changed(&mut self, pane: PaneId, before: Summary) -> bool {
        let viewed = self.viewed(pane);
        let state = self.panes.get_mut(&pane).unwrap();
        if state.agents.is_empty() {
            state.agent_program = None;
        }
        let after = summary_key(&state.agents);
        if after == before {
            return false;
        }
        state.unseen = after.is_some_and(|(state, _)| alerting(state)) && !viewed;
        state.unseen
    }

    fn notify(&mut self, now: Millis, pane: PaneId, out: &mut Vec<Effect>) {
        let quiet = self.panes[&pane]
            .notified
            .is_some_and(|last| now.saturating_sub(last) < NOTIFY_INTERVAL);
        if quiet
            || !self.config.notifications
            || !self
                .clients
                .values()
                .any(|client| client.kind == ClientKind::Gui)
        {
            return;
        }
        if let Some(effect) = self.notification(pane) {
            out.push(effect);
            self.panes.get_mut(&pane).unwrap().notified = Some(now);
        }
    }

    fn notification(&self, pane: PaneId) -> Option<Effect> {
        let (session, window) = self.locate(pane)?;
        let row = self.agent_row(session, window, pane)?;
        let (_, record) = summary(&self.panes[&pane].agents)?;
        let app = &row.app;
        let label = row.state.label(row.kind).to_lowercase();
        let session = &row.session;
        let text = record.msg.as_deref().unwrap_or(&row.title);
        Some(Effect::Notify {
            title: format!("{app} {label}"),
            body: escape(&format!("{session}: {text}")),
        })
    }

    fn locate(&self, pane: PaneId) -> Option<(&Session, &Window)> {
        self.tree.sessions.iter().find_map(|session| {
            session
                .windows
                .iter()
                .find(|window| window.panes.contains(&pane))
                .map(|window| (session, window))
        })
    }

    fn viewed(&self, pane: PaneId) -> bool {
        let Some((_, window)) = self.locate(pane) else {
            return false;
        };
        self.clients.values().any(|client| {
            client.kind == ClientKind::Gui && client.focused && client.view.window == window.id
        })
    }

    pub(super) fn see_agents(&mut self) {
        let seen: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|(_, state)| state.unseen)
            .map(|(pane, _)| *pane)
            .filter(|pane| self.viewed(*pane))
            .collect();
        for pane in seen {
            self.panes.get_mut(&pane).unwrap().unseen = false;
        }
    }

    pub(super) fn agent_program(&mut self, pane: PaneId, program: String) {
        let Some(state) = self.panes.get_mut(&pane) else {
            return;
        };
        if state.agents.is_empty() {
            return;
        }
        let changed = state
            .agent_program
            .as_ref()
            .is_some_and(|known| *known != program);
        if changed {
            self.drop_agents(pane);
        }
        let state = self.panes.get_mut(&pane).unwrap();
        if !state.agents.is_empty() {
            state.agent_program = Some(program);
        }
    }

    pub(super) fn agent_rows(&self) -> Vec<AgentRow> {
        self.tree
            .sessions
            .iter()
            .flat_map(|session| {
                session.windows.iter().flat_map(move |window| {
                    window
                        .panes
                        .iter()
                        .filter_map(move |pane| self.agent_row(session, window, *pane))
                })
            })
            .collect()
    }

    fn agent_row(&self, session: &Session, window: &Window, pane: PaneId) -> Option<AgentRow> {
        let state = self.panes.get(&pane)?;
        let (_, record) = summary(&state.agents)?;
        let root = state.agents.get("");
        let (program, label) = self.process(pane);
        Some(AgentRow {
            session: session.name.clone(),
            window: window.id,
            pane,
            app: record
                .app
                .clone()
                .or_else(|| root?.app.clone())
                .unwrap_or(program),
            state: record.state,
            kind: record.kind,
            title: record
                .title
                .clone()
                .or_else(|| root?.title.clone())
                .or_else(|| window.name.clone())
                .unwrap_or(label),
            unseen: state.unseen,
        })
    }
}

#[cfg(test)]
#[path = "server_agents_tests.rs"]
mod tests;
