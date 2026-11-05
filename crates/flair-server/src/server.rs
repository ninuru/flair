use flair_core::{
    command::Command,
    ids::{ClientId, PaneId, SessionId, WindowId},
    key::{InputEvent, MouseAction, MouseButton},
    proto::{ClientMsg, ExitReason, PROTOCOL, ServerMsg, Target},
    time::Millis,
    view::ClientView,
};
use flair_term::{
    TermEvent, Terminal,
    encode::{encode_focus, encode_key, encode_mouse, encode_paste, encode_text},
    pty::SpawnSpec,
    terminal::MouseMode,
};

use crate::{client::ClientState, config::ServerConfig, effect::Effect, event::Event, pane::Pane};

pub struct Server {
    config: ServerConfig,
    pane: Option<Pane>,
    client: Option<ClientState>,
    exited: bool,
}

impl Server {
    pub fn new(config: ServerConfig) -> Server {
        Server {
            config,
            pane: None,
            client: None,
            exited: false,
        }
    }

    pub fn handle(&mut self, now: Millis, event: Event, out: &mut Vec<Effect>) {
        match event {
            Event::ClientConnected(_) => {}
            Event::Client(id, message) => self.client_message(now, id, message, out),
            Event::ClientGone(id) => {
                if self.client.as_ref().is_some_and(|client| client.id == id) {
                    self.client = None;
                }
            }
            Event::PtyOutput(id, bytes) => {
                if id == PaneId(1) {
                    if let Some(pane) = &mut self.pane {
                        pane.terminal.advance(&bytes);
                        pane.dirty = true;
                    }
                    self.terminal_events(out);
                }
            }
            Event::PtyExited(id) => {
                if id == PaneId(1) {
                    self.exited = true;
                }
            }
            Event::Timer(_) => {}
        }
        self.flush(out);
    }

    fn client_message(
        &mut self,
        now: Millis,
        id: ClientId,
        message: ClientMsg,
        out: &mut Vec<Effect>,
    ) {
        if let ClientMsg::Hello {
            protocol,
            kind: _,
            size,
            palette,
            env,
            target,
        } = message
        {
            if protocol != PROTOCOL || size.cols == 0 || size.rows == 0 {
                out.push(Effect::Send(
                    id,
                    ServerMsg::Error("incompatible protocol or invalid grid size".into()),
                ));
                return;
            }
            if self.client.is_some() {
                out.push(Effect::Send(
                    id,
                    ServerMsg::Error("M0 supports one attached client".into()),
                ));
                return;
            }
            if self.pane.is_none() {
                let mut terminal = Terminal::new(size, self.config.scrollback);
                terminal.set_palette(&palette);
                self.pane = Some(Pane {
                    terminal,
                    size,
                    dirty: true,
                    full: true,
                });
                let cwd = match target {
                    Target::Path(path) => path,
                    Target::Main | Target::New | Target::Name(_) => self.config.cwd.clone(),
                };
                let mut pane_env = self.config.env.clone();
                for (key, value) in env {
                    if let Some(existing) = pane_env.iter_mut().find(|(name, _)| name == &key) {
                        existing.1 = value;
                    } else {
                        pane_env.push((key, value));
                    }
                }
                out.push(Effect::Spawn(
                    PaneId(1),
                    SpawnSpec {
                        program: self.config.program.clone(),
                        args: self.config.args.clone(),
                        cwd,
                        env: pane_env,
                        size,
                    },
                ));
            }
            self.client = Some(ClientState {
                id,
                view: ClientView::new(SessionId(1), WindowId(1), now),
                size,
                awaiting_ack: None,
                next_seq: 1,
            });
            if let Some(pane) = &mut self.pane {
                pane.full = true;
                pane.dirty = true;
            }
            out.push(Effect::Send(id, ServerMsg::Welcome { client: id }));
            return;
        }
        if !self.client.as_ref().is_some_and(|client| client.id == id) {
            return;
        }
        match message {
            ClientMsg::Hello { .. } => unreachable!(),
            ClientMsg::Resize(size) => {
                if size.cols == 0 || size.rows == 0 {
                    return;
                }
                if let Some(client) = &mut self.client {
                    client.size = size;
                }
                if let Some(pane) = &mut self.pane {
                    pane.size = size;
                    pane.terminal.resize(size);
                    pane.full = true;
                    pane.dirty = true;
                    out.push(Effect::ResizePty(PaneId(1), size));
                }
            }
            ClientMsg::Input(input) => {
                if let Some(client) = &mut self.client {
                    client.view.touch(now);
                }
                self.input(input, out);
            }
            ClientMsg::Focus(focused) => {
                if let Some(client) = &mut self.client
                    && focused
                {
                    client.view.touch(now);
                }
                if let Some(pane) = &self.pane
                    && let Some(bytes) = encode_focus(focused, &pane.terminal.modes())
                {
                    out.push(Effect::WritePty(PaneId(1), bytes));
                }
            }
            ClientMsg::FrameAck { seq } => {
                if let Some(client) = &mut self.client
                    && client.awaiting_ack == Some(seq)
                {
                    client.awaiting_ack = None;
                }
            }
            ClientMsg::Command(command) => {
                if let Some(pane) = &mut self.pane {
                    match command {
                        Command::ScrollLines(lines) => pane.terminal.scroll(lines),
                        Command::ScrollPages(pages) => pane
                            .terminal
                            .scroll(pages.saturating_mul(i32::from(pane.size.rows))),
                        Command::ScrollToBottom => pane.terminal.scroll_to_bottom(),
                    }
                    pane.dirty = true;
                    pane.full = true;
                }
            }
            ClientMsg::Detach => {
                self.client = None;
                out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::Detached)));
                out.push(Effect::KillPty(PaneId(1)));
                out.push(Effect::Shutdown);
            }
        }
    }

    fn input(&mut self, input: InputEvent, out: &mut Vec<Effect>) {
        let Some(pane) = &mut self.pane else {
            return;
        };
        let modes = pane.terminal.modes();
        let bytes = match input {
            InputEvent::Key(key) => {
                pane.terminal.scroll_to_bottom();
                pane.full = true;
                pane.dirty = true;
                encode_key(&key, &modes)
            }
            InputEvent::Text(text) => {
                pane.terminal.scroll_to_bottom();
                pane.full = true;
                pane.dirty = true;
                Some(encode_text(&text))
            }
            InputEvent::Paste(text) => {
                pane.terminal.scroll_to_bottom();
                pane.full = true;
                pane.dirty = true;
                Some(encode_paste(&text, &modes))
            }
            InputEvent::Mouse(mouse) => {
                if modes.mouse == MouseMode::Off && mouse.action == MouseAction::Press {
                    let lines = match mouse.button {
                        MouseButton::WheelUp => 3,
                        MouseButton::WheelDown => -3,
                        MouseButton::Left
                        | MouseButton::Middle
                        | MouseButton::Right
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                        | MouseButton::NoButton => 0,
                    };
                    if lines != 0 {
                        pane.terminal.scroll(lines);
                        pane.full = true;
                        pane.dirty = true;
                    }
                    None
                } else {
                    encode_mouse(&mouse, &modes)
                }
            }
        };
        if let Some(bytes) = bytes
            && !bytes.is_empty()
        {
            out.push(Effect::WritePty(PaneId(1), bytes));
        }
    }

    fn terminal_events(&mut self, out: &mut Vec<Effect>) {
        let Some(pane) = &mut self.pane else {
            return;
        };
        for event in pane.terminal.take_events() {
            match event {
                TermEvent::PtyWrite(bytes) => out.push(Effect::WritePty(PaneId(1), bytes)),
                TermEvent::Title(title) => {
                    if let Some(client) = &self.client {
                        out.push(Effect::Send(client.id, ServerMsg::Title(title)));
                    }
                }
                TermEvent::Bell => {
                    if let Some(client) = &self.client {
                        out.push(Effect::Send(client.id, ServerMsg::Bell));
                    }
                }
                TermEvent::Cwd(path) => log::debug!("terminal cwd: {}", path.display()),
                TermEvent::Mark(kind, status) => log::debug!("terminal mark: {kind:?} {status:?}"),
            }
        }
    }

    fn flush(&mut self, out: &mut Vec<Effect>) {
        let Some(client) = &mut self.client else {
            return;
        };
        if client.awaiting_ack.is_some() {
            return;
        }
        if let Some(pane) = &mut self.pane
            && pane.dirty
        {
            if let Some(update) = pane.terminal.snapshot(PaneId(1), pane.full) {
                let seq = client.next_seq;
                client.next_seq = client.next_seq.saturating_add(1);
                client.awaiting_ack = Some(seq);
                out.push(Effect::Send(
                    client.id,
                    ServerMsg::Frame {
                        seq,
                        panes: vec![update],
                    },
                ));
            }
            pane.dirty = false;
            pane.full = false;
        }
        if self.exited && client.awaiting_ack.is_none() {
            out.push(Effect::Send(
                client.id,
                ServerMsg::Exit(ExitReason::SessionEnded),
            ));
            out.push(Effect::Shutdown);
            self.client = None;
        }
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
