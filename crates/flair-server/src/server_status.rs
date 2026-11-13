use super::*;
use flair_core::{
    key::{InputEvent, KeyInput},
    prefix::PrefixResult,
    status::{StatusLine, StatusWindow},
    tree::Session,
};

use crate::process::process_name;

const PROCESS_SETTLE: Millis = Millis(150);
const PROCESS_INTERVAL: Millis = Millis(1000);

impl Server {
    pub(super) fn status_line(&self, id: ClientId) -> Option<StatusLine> {
        let client = self.clients.get(&id)?;
        let session = self.tree.session(client.view.session)?;
        let windows = self.window_entries(session, client.view.window, client.view.last_window);
        Some(StatusLine {
            session: session.name.clone(),
            hidden: u16::try_from(self.tree.sessions.len() - 1).unwrap_or(u16::MAX),
            running: self
                .tree
                .sessions
                .iter()
                .any(|other| other.id != session.id && self.session_running(other.id)),
            finished: client
                .finished
                .iter()
                .any(|other| *other != session.id && self.tree.session(*other).is_some()),
            windows,
            pending: client
                .prefix
                .pending()
                .then(|| prefix_label(&self.config.keys.key)),
            copy: self
                .tree
                .active_pane(client.view.window)
                .and_then(|pane| self.panes.get(&pane))
                .is_some_and(|pane| pane.terminal.copy_mode()),
        })
    }

    pub(super) fn window_entries(
        &self,
        session: &Session,
        active: WindowId,
        last: Option<WindowId>,
    ) -> Vec<StatusWindow> {
        session
            .windows
            .iter()
            .enumerate()
            .map(|(index, window)| {
                let (program, label) = self.process(window.active_pane);
                StatusWindow {
                    id: window.id,
                    index: u16::try_from(index + 1).unwrap_or(u16::MAX),
                    name: window.name.clone().unwrap_or(label),
                    program,
                    active: window.id == active,
                    last: Some(window.id) == last,
                }
            })
            .collect()
    }

    fn process(&self, pane: PaneId) -> (String, String) {
        self.panes
            .get(&pane)
            .and_then(|pane| pane.process.clone())
            .filter(|(program, _)| !program.is_empty())
            .unwrap_or_else(|| {
                let program = self
                    .config
                    .program
                    .rsplit('/')
                    .next()
                    .unwrap_or(&self.config.program)
                    .to_owned();
                (program.clone(), program)
            })
    }

    pub(super) fn want_process(&mut self, now: Millis, pane: PaneId, out: &mut Vec<Effect>) {
        let Some(state) = self.panes.get_mut(&pane) else {
            return;
        };
        if state.exited || state.probe_due.is_some() {
            return;
        }
        let soonest = now + PROCESS_SETTLE;
        let due = state
            .probed
            .map_or(soonest, |last| soonest.max(last + PROCESS_INTERVAL));
        state.probe_due = Some(due);
        out.push(Effect::SetTimer(Timer::Process(pane), due));
    }

    pub(super) fn process_timer(&mut self, now: Millis, pane: PaneId, out: &mut Vec<Effect>) {
        let Some(state) = self.panes.get_mut(&pane) else {
            return;
        };
        if state.probe_due.take().is_some() && !state.exited {
            state.probed = Some(now);
            out.push(Effect::ReadProcess(pane));
        }
    }

    pub(super) fn process_read(&mut self, pane: PaneId, argv: &[String]) {
        if let Some(state) = self.panes.get_mut(&pane) {
            state.process = Some(process_name(argv));
        }
    }

    fn session_running(&self, session: SessionId) -> bool {
        self.tree.session(session).is_some_and(|session| {
            session
                .windows
                .iter()
                .flat_map(|window| &window.panes)
                .any(|pane| {
                    self.panes
                        .get(pane)
                        .is_some_and(|pane| pane.started.is_some())
                })
        })
    }

    pub(super) fn set_started(&mut self, pane: PaneId, started: Option<Millis>) {
        let session = self
            .tree
            .sessions
            .iter()
            .find(|session| {
                session
                    .windows
                    .iter()
                    .any(|window| window.panes.contains(&pane))
            })
            .map(|session| session.id);
        let before = session.is_some_and(|session| self.session_running(session));
        if let Some(state) = self.panes.get_mut(&pane) {
            state.started = started;
        }
        if let Some(session) = session
            && before
            && !self.session_running(session)
        {
            for client in self.clients.values_mut() {
                if client.view.session != session {
                    client.finished.insert(session);
                }
            }
        }
    }

    pub(super) fn flush_status(&mut self, out: &mut Vec<Effect>) {
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            let client = self.clients.get_mut(&id).unwrap();
            let current = client.view.session;
            client.finished.remove(&current);
            let Some(line) = self.status_line(id) else {
                continue;
            };
            let client = self.clients.get_mut(&id).unwrap();
            if client.status.as_ref() != Some(&line) {
                client.status = Some(line.clone());
                out.push(Effect::Send(id, ServerMsg::Status(line)));
            }
        }
    }

    pub(super) fn key_input(
        &mut self,
        now: Millis,
        id: ClientId,
        key: KeyInput,
        out: &mut Vec<Effect>,
    ) {
        let client = self.clients.get_mut(&id).unwrap();
        let before = client.prefix.deadline();
        let result = client.prefix.handle(&key, now, &self.config.keys);
        self.prefix_changed(id, before, out);
        match result {
            PrefixResult::Forward | PrefixResult::SendPrefix => {
                if self.copy_pane(id).is_some() {
                    self.copy_key(now, id, &key, out);
                } else {
                    self.forward(id, InputEvent::Key(key), out);
                }
            }
            PrefixResult::Consume => {}
            PrefixResult::Command(command) => self.command(now, id, command, out),
        }
    }

    pub(super) fn set_ime(&mut self, id: ClientId, composing: bool, out: &mut Vec<Effect>) {
        let client = self.clients.get_mut(&id).unwrap();
        let before = client.prefix.deadline();
        client.prefix.set_ime(composing);
        self.prefix_changed(id, before, out);
    }

    pub(super) fn cancel_prefix(&mut self, id: ClientId, out: &mut Vec<Effect>) {
        let Some(client) = self.clients.get_mut(&id) else {
            return;
        };
        if client.prefix.pending() {
            let before = client.prefix.deadline();
            client.prefix.reset();
            self.prefix_changed(id, before, out);
        }
    }

    fn prefix_changed(
        &mut self,
        id: ClientId,
        before: Option<(u64, Millis)>,
        out: &mut Vec<Effect>,
    ) {
        let after = self.clients[&id].prefix.deadline();
        if after == before {
            return;
        }
        if let Some((generation, until)) = after {
            out.push(Effect::SetTimer(
                Timer::Prefix {
                    client: id,
                    generation,
                },
                until,
            ));
        }
        if before.is_some() != after.is_some() {
            self.send_title(id, out);
        }
    }

    pub(super) fn prefix_timer(
        &mut self,
        now: Millis,
        id: ClientId,
        generation: u64,
        out: &mut Vec<Effect>,
    ) {
        let Some(client) = self.clients.get_mut(&id) else {
            return;
        };
        if client.prefix.expire(generation, now) {
            self.send_title(id, out);
        }
    }

    pub(super) fn reload_config(&self, out: &mut Vec<Effect>) {
        out.push(Effect::ReadConfig(self.config.path.clone()));
    }

    pub(super) fn config_read(&mut self, result: Result<String, String>, out: &mut Vec<Effect>) {
        let text = match result {
            Ok(text) => text,
            Err(error) => {
                log::warn!("config reload failed, keeping the current config: {error}");
                return;
            }
        };
        match Config::from_toml(&text) {
            Ok(config) => self.config = self.config.reloaded(&config),
            Err(error) => {
                log::warn!("config reload failed, keeping the current config: {error}");
            }
        }
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            let client = self.clients.get_mut(&id).unwrap();
            let before = client.prefix.deadline();
            client.prefix.reset();
            self.prefix_changed(id, before, out);
            out.push(Effect::Send(
                id,
                ServerMsg::ReloadConfig { toml: text.clone() },
            ));
        }
    }
}

#[cfg(test)]
#[path = "server_status_tests.rs"]
mod tests;
