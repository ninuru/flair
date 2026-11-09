use std::collections::{BTreeMap, BTreeSet};

use flair_core::{
    config::Config,
    geom::GridSize,
    ids::{ClientId, PaneId, SessionId, WindowId},
    prefix::PrefixMachine,
    proto::{ClientKind, ClientMsg, Control, ExitReason, PROTOCOL, ServerMsg, Target},
    time::Millis,
    tree::Tree,
    view::ClientView,
};
use flair_term::{MarkKind, TermEvent, encode::encode_focus};

use crate::{client::ClientState, config::ServerConfig, effect::Effect, event::Event, pane::Pane};

#[path = "server_copy.rs"]
mod copy;
#[path = "server_frames.rs"]
mod frames;
#[path = "server_input.rs"]
mod input;
#[path = "server_lifecycle.rs"]
mod lifecycle;
#[path = "server_mouse.rs"]
mod mouse;
#[path = "server_sessions.rs"]
mod sessions;
#[path = "server_status.rs"]
mod status;
#[path = "server_windows.rs"]
mod windows;

pub struct Server {
    config: ServerConfig,
    tree: Tree,
    panes: BTreeMap<PaneId, Pane>,
    clients: BTreeMap<ClientId, ClientState>,
    next_id: u32,
    activity: u64,
    stopping: bool,
}

impl Server {
    pub fn new(config: ServerConfig) -> Server {
        Server {
            config,
            tree: Tree { sessions: vec![] },
            panes: BTreeMap::new(),
            clients: BTreeMap::new(),
            next_id: 1,
            activity: 0,
            stopping: false,
        }
    }

    pub fn handle(&mut self, now: Millis, event: Event, out: &mut Vec<Effect>) {
        if self.stopping {
            return;
        }
        match event {
            Event::ClientConnected(_) => {}
            Event::Client(id, message) => self.client_message(now, id, message, out),
            Event::ClientGone(id) => {
                self.remove_client(id, out);
            }
            Event::PtyOutput(id, bytes) => {
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.terminal.advance(&bytes);
                    pane.dirty = true;
                    self.terminal_events(id, out);
                }
            }
            Event::PtySpawnFailed(id) => {
                for (client_id, client) in &mut self.clients {
                    if self.tree.active_pane(client.view.window) == Some(id) {
                        client.awaiting_ack = None;
                        client.previous.clear();
                        out.push(Effect::Send(
                            *client_id,
                            ServerMsg::Error("could not start shell".into()),
                        ));
                    }
                }
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.exited = true;
                    pane.dirty = false;
                    pane.cached = None;
                }
                self.set_running(id, false);
            }
            Event::PtyExited(id) => {
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.exited = true;
                }
                self.set_running(id, false);
            }
            Event::Timer(timer) => self.prefix_timer(now, timer, out),
            Event::ConfigRead(result) => self.config_read(result, out),
        }
        if !self.stopping {
            self.reap_sessions(now, out);
            self.flush_frames(out);
            self.flush_status(out);
        }
    }

    fn client_message(
        &mut self,
        now: Millis,
        id: ClientId,
        message: ClientMsg,
        out: &mut Vec<Effect>,
    ) {
        match message {
            ClientMsg::Hello {
                protocol,
                kind,
                size,
                palette,
                env,
                target,
            } => {
                if protocol != PROTOCOL || !valid_size(size) || self.clients.contains_key(&id) {
                    out.push(Effect::Send(
                        id,
                        ServerMsg::Error(
                            "incompatible protocol, invalid grid size, or repeated hello".into(),
                        ),
                    ));
                    return;
                }
                let session = match self.resolve_target(target, size, &palette, &env, out) {
                    Ok(session) => session,
                    Err(error) => {
                        out.push(Effect::Send(id, ServerMsg::Error(error)));
                        return;
                    }
                };
                let window = self.tree.session(session).unwrap().last_window;
                self.activity += 1;
                self.clients.insert(
                    id,
                    ClientState {
                        view: ClientView::new(session, window, now),
                        kind,
                        size,
                        awaiting_ack: None,
                        next_seq: 1,
                        activity: self.activity,
                        focused: false,
                        full: true,
                        previous: BTreeMap::new(),
                        acknowledged: BTreeMap::new(),
                        layout: None,
                        env,
                        palette,
                        prefix: PrefixMachine::default(),
                        status: None,
                        finished: BTreeSet::new(),
                        last_click: None,
                        selecting: false,
                        mouse: None,
                    },
                );
                out.push(Effect::Send(id, ServerMsg::Welcome { client: id }));
                self.resize_window(window, out);
                self.send_title(id, out);
            }
            ClientMsg::Control(control) => self.control(now, id, control, out),
            message => {
                if !self.clients.contains_key(&id) {
                    out.push(Effect::Send(
                        id,
                        ServerMsg::Error("send Hello before client messages".into()),
                    ));
                    return;
                }
                self.attached_message(now, id, message, out);
            }
        }
    }

    fn attached_message(
        &mut self,
        now: Millis,
        id: ClientId,
        message: ClientMsg,
        out: &mut Vec<Effect>,
    ) {
        match message {
            ClientMsg::Resize(size) => {
                if valid_size(size) {
                    let client = self.clients.get_mut(&id).unwrap();
                    client.size = size;
                    let window = client.view.window;
                    self.resize_window(window, out);
                }
            }
            ClientMsg::Input(input) => {
                self.touch(now, id, out);
                self.input(now, id, input, out);
            }
            ClientMsg::Focus(focused) => {
                if !focused {
                    self.cancel_mouse(id, out);
                }
                self.clients.get_mut(&id).unwrap().focused = focused;
                if focused {
                    self.touch(now, id, out);
                } else {
                    self.cancel_prefix(id, out);
                }
                if let Some(pane) = self.client_pane(id)
                    && let Some(bytes) = encode_focus(focused, &self.panes[&pane].terminal.modes())
                {
                    out.push(Effect::WritePty(pane, bytes));
                }
            }
            ClientMsg::FrameAck { seq } => {
                let client = self.clients.get_mut(&id).unwrap();
                if client.awaiting_ack == Some(seq) {
                    client.acknowledged = client.previous.clone();
                    client.awaiting_ack = None;
                }
            }
            ClientMsg::Command(command) => {
                self.touch(now, id, out);
                self.command(now, id, command, out);
            }
            ClientMsg::Detach => {
                self.remove_client(id, out);
                out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::Detached)));
            }
            ClientMsg::Hello { .. } | ClientMsg::Control(_) => unreachable!(),
        }
    }

    fn touch(&mut self, now: Millis, id: ClientId, out: &mut Vec<Effect>) {
        self.activity += 1;
        let client = self.clients.get_mut(&id).unwrap();
        client.view.touch(now);
        client.activity = self.activity;
        let window = client.view.window;
        let session = client.view.session;
        self.tree
            .sessions
            .iter_mut()
            .find(|candidate| candidate.id == session)
            .unwrap()
            .last_window = window;
        self.resize_window(window, out);
    }

    pub(super) fn remove_client(&mut self, id: ClientId, out: &mut Vec<Effect>) {
        if self.clients.contains_key(&id) {
            self.cancel_mouse(id, out);
        }
        if let Some(client) = self.clients.remove(&id) {
            self.resize_window(client.view.window, out);
        }
    }

    fn client_pane(&self, id: ClientId) -> Option<PaneId> {
        self.clients
            .get(&id)
            .and_then(|client| self.tree.active_pane(client.view.window))
    }

    fn terminal_events(&mut self, pane: PaneId, out: &mut Vec<Effect>) {
        let events = self.panes.get_mut(&pane).unwrap().terminal.take_events();
        for event in events {
            match event {
                TermEvent::PtyWrite(bytes) => out.push(Effect::WritePty(pane, bytes)),
                TermEvent::Title(_) => {
                    let clients: Vec<_> = self
                        .clients
                        .keys()
                        .copied()
                        .filter(|id| self.client_pane(*id) == Some(pane))
                        .collect();
                    for id in clients {
                        self.send_title(id, out);
                    }
                }
                TermEvent::Bell => {
                    for id in self
                        .clients
                        .keys()
                        .copied()
                        .filter(|id| self.client_pane(*id) == Some(pane))
                    {
                        out.push(Effect::Send(id, ServerMsg::Bell));
                    }
                }
                TermEvent::Mark(MarkKind::CommandStart, _) => self.set_running(pane, true),
                TermEvent::Mark(MarkKind::CommandEnd | MarkKind::PromptStart, _) => {
                    self.set_running(pane, false);
                }
                TermEvent::Cwd(_) | TermEvent::Mark(MarkKind::PromptEnd, _) => {}
            }
        }
    }

    fn send_title(&self, id: ClientId, out: &mut Vec<Effect>) {
        let client = &self.clients[&id];
        let title = self
            .client_pane(id)
            .and_then(|pane| self.panes[&pane].terminal.title())
            .unwrap_or_else(|| {
                self.tree
                    .session(client.view.session)
                    .map_or("flair", |session| session.name.as_str())
            });
        let title = if client.prefix.pending() {
            format!("{title} {}", prefix_label(&self.config.keys.key))
        } else {
            title.to_owned()
        };
        out.push(Effect::Send(id, ServerMsg::Title(title)));
    }
}

fn prefix_label(key: &str) -> String {
    match key.strip_prefix("C-").map(|rest| {
        let mut chars = rest.chars();
        (chars.next(), chars.next())
    }) {
        Some((Some(c), None)) => format!("^{}", c.to_ascii_uppercase()),
        Some(_) | None => key.to_owned(),
    }
}

fn valid_size(size: GridSize) -> bool {
    size.cols > 0 && size.rows > 0 && u32::from(size.cols) * u32::from(size.rows) <= 1_000_000
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
