use super::*;
use flair_core::{
    key::{InputEvent, KeyInput},
    prefix::PrefixResult,
    status::{StatusLine, StatusWindow},
};

impl Server {
    pub(super) fn status_line(&self, id: ClientId) -> Option<StatusLine> {
        let client = self.clients.get(&id)?;
        let session = self.tree.session(client.view.session)?;
        let windows = session
            .windows
            .iter()
            .enumerate()
            .map(|(index, window)| StatusWindow {
                id: window.id,
                index: u16::try_from(index + 1).unwrap_or(u16::MAX),
                name: window
                    .name
                    .clone()
                    .or_else(|| {
                        self.panes
                            .get(&window.active_pane)
                            .and_then(|pane| pane.terminal.title())
                            .and_then(window_label)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| self.program_name()),
                active: window.id == client.view.window,
                last: Some(window.id) == client.view.last_window,
            })
            .collect();
        Some(StatusLine {
            session: session.name.clone(),
            windows,
            pending: client
                .prefix
                .pending()
                .then(|| prefix_label(&self.config.keys.key)),
        })
    }

    fn program_name(&self) -> String {
        self.config
            .program
            .rsplit('/')
            .next()
            .unwrap_or(&self.config.program)
            .to_owned()
    }

    pub(super) fn flush_status(&mut self, out: &mut Vec<Effect>) {
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
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
                self.forward(id, InputEvent::Key(key), out)
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
            out.push(Effect::SetTimer(timer_token(id, generation), until));
        }
        if before.is_some() != after.is_some() {
            self.send_title(id, out);
        }
    }

    pub(super) fn prefix_timer(&mut self, now: Millis, token: u64, out: &mut Vec<Effect>) {
        let id = ClientId((token >> 32) as u32);
        let Some(client) = self.clients.get_mut(&id) else {
            return;
        };
        let Some((generation, _)) = client.prefix.deadline() else {
            return;
        };
        if timer_token(id, generation) == token && client.prefix.expire(generation, now) {
            self.send_title(id, out);
        }
    }

    pub(super) fn reload_config(&self, out: &mut Vec<Effect>) {
        out.push(Effect::ReadConfig(self.config.path.clone()));
    }

    pub(super) fn config_read(&mut self, result: Result<String, String>, out: &mut Vec<Effect>) {
        let parsed = result.and_then(|text| {
            Config::from_toml(&text)
                .map(|config| (text, config))
                .map_err(|error| error.to_string())
        });
        let (text, config) = match parsed {
            Ok(value) => value,
            Err(error) => {
                log::warn!("config reload failed, keeping the current config: {error}");
                return;
            }
        };
        self.config = self.config.reloaded(&config);
        let palette = config.theme();
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            let client = self.clients.get_mut(&id).unwrap();
            client.palette = palette.clone();
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

fn window_label(title: &str) -> Option<&str> {
    let title = title.trim();
    let path = match title.split_once(':') {
        Some((prompt, path)) if prompt.contains('@') && !prompt.contains(char::is_whitespace) => {
            Some(path.trim_start())
        }
        Some(_) | None => {
            (title.starts_with(['~', '/']) && !title.contains(char::is_whitespace)).then_some(title)
        }
    };
    let label = match path {
        Some(path) if path.starts_with('/') && path.trim_matches('/').is_empty() => "/",
        Some(path) => path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default(),
        None => title
            .split_whitespace()
            .next()
            .and_then(|word| word.rsplit('/').next())
            .unwrap_or_default(),
    };
    (!label.is_empty()).then_some(label)
}

fn timer_token(id: ClientId, generation: u64) -> u64 {
    (u64::from(id.0) << 32) | (generation & u64::from(u32::MAX))
}

#[cfg(test)]
#[path = "server_status_tests.rs"]
mod tests;
