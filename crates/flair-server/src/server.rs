use std::collections::BTreeMap;

use flair_core::{
    geom::GridSize,
    ids::{ClientId, PaneId, SessionId, WindowId},
    proto::{ClientKind, ClientMsg, Control, ExitReason, PROTOCOL, ServerMsg, Target},
    time::Millis,
    tree::Tree,
    view::ClientView,
};
use flair_term::{TermEvent, encode::encode_focus};

use crate::{client::ClientState, config::ServerConfig, effect::Effect, event::Event, pane::Pane};

#[path = "server_input.rs"]
mod input;
#[path = "server_sessions.rs"]
mod sessions;

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
                self.clients.remove(&id);
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
                        client.previous = None;
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
            }
            Event::PtyExited(id) => {
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.exited = true;
                }
            }
            Event::Timer(timer) => {
                if let Ok(id) = u32::try_from(timer) {
                    let id = ClientId(id);
                    if self
                        .clients
                        .get(&id)
                        .and_then(|c| c.prefix_until)
                        .is_some_and(|until| now >= until)
                    {
                        self.clients.get_mut(&id).unwrap().prefix_until = None;
                        self.send_title(id, out);
                    }
                }
            }
        }
        if !self.stopping {
            self.flush_frames(out);
            self.reap_sessions(now, out);
            self.flush_frames(out);
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
                let window = self.tree.session(session).unwrap().windows[0].id;
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
                        previous: None,
                        env,
                        palette,
                        prefix_until: None,
                        last_click: None,
                        selecting: false,
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
                self.clients.get_mut(&id).unwrap().focused = focused;
                if focused {
                    self.touch(now, id, out);
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
                    client.awaiting_ack = None;
                }
            }
            ClientMsg::Command(command) => self.command(now, id, command, out),
            ClientMsg::Detach => {
                self.clients.remove(&id);
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
        self.resize_window(window, out);
    }

    fn client_pane(&self, id: ClientId) -> Option<PaneId> {
        self.clients
            .get(&id)
            .and_then(|client| self.tree.active_pane(client.view.window))
    }

    fn resize_window(&mut self, window: WindowId, out: &mut Vec<Effect>) {
        let Some(size) = self
            .clients
            .values()
            .filter(|client| client.view.window == window)
            .max_by_key(|client| client.activity)
            .map(|client| client.size)
        else {
            return;
        };
        let Some(id) = self.tree.active_pane(window) else {
            return;
        };
        let pane = self.panes.get_mut(&id).unwrap();
        if pane.size != size {
            pane.size = size;
            pane.terminal.resize(size);
            pane.dirty = true;
            for client in self
                .clients
                .values_mut()
                .filter(|client| client.view.window == window)
            {
                client.full = true;
            }
            out.push(Effect::ResizePty(id, size));
        }
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
                TermEvent::Cwd(_) | TermEvent::Mark(_, _) => {}
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
        let title = if client.prefix_until.is_some() {
            format!("{title} ^A")
        } else {
            title.to_owned()
        };
        out.push(Effect::Send(id, ServerMsg::Title(title)));
    }

    fn flush_frames(&mut self, out: &mut Vec<Effect>) {
        for (id, pane) in &mut self.panes {
            if pane.dirty {
                pane.cached = pane.terminal.snapshot(*id, true);
                pane.dirty = false;
            }
        }
        for (id, client) in &mut self.clients {
            if client.awaiting_ack.is_some() {
                continue;
            }
            let Some(pane) = self
                .tree
                .active_pane(client.view.window)
                .and_then(|pane| self.panes.get(&pane))
            else {
                continue;
            };
            let Some(current) = &pane.cached else {
                continue;
            };
            if !client.full && client.previous.as_ref() == Some(current) {
                continue;
            }
            let mut update = current.clone();
            if !client.full
                && let Some(previous) = &client.previous
                && previous.pane == current.pane
                && previous.size == current.size
                && previous.flags == current.flags
            {
                update.full = false;
                update
                    .rows
                    .retain(|(row, cells)| previous.rows[usize::from(*row)].1 != *cells);
            }
            client.previous = Some(current.clone());
            client.full = false;
            let seq = client.next_seq;
            client.next_seq += 1;
            client.awaiting_ack = Some(seq);
            out.push(Effect::Send(
                *id,
                ServerMsg::Frame {
                    seq,
                    panes: vec![update],
                },
            ));
        }
    }

    fn reap_sessions(&mut self, now: Millis, out: &mut Vec<Effect>) {
        let finished: Vec<_> = self
            .panes
            .iter()
            .filter(|(pane_id, pane)| {
                pane.exited
                    && self
                        .clients
                        .iter()
                        .filter(|(id, _)| self.client_pane(**id) == Some(**pane_id))
                        .all(|(_, client)| {
                            client.awaiting_ack.is_none()
                                && client.previous.as_ref() == pane.cached.as_ref()
                        })
            })
            .map(|(id, _)| *id)
            .collect();
        for pane in finished {
            self.panes.remove(&pane);
            out.push(Effect::KillPty(pane));
            let removed = self.tree.remove_pane(pane);
            if let Some(session) = removed.session {
                let affected: Vec<_> = self
                    .clients
                    .iter()
                    .filter(|(_, client)| client.view.session == session)
                    .map(|(id, _)| *id)
                    .collect();
                for id in affected {
                    let last = self.clients[&id]
                        .view
                        .last_session
                        .filter(|last| self.tree.session(*last).is_some());
                    if let Some(last) = last {
                        self.switch_to(now, id, last, out);
                    } else {
                        self.clients.remove(&id);
                        out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::SessionEnded)));
                    }
                }
            }
        }
        if self.next_id > 1 && self.tree.sessions.is_empty() {
            self.stopping = true;
            out.push(Effect::Shutdown);
        }
    }
}

fn valid_size(size: GridSize) -> bool {
    size.cols > 0 && size.rows > 0 && u32::from(size.cols) * u32::from(size.rows) <= 1_000_000
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
