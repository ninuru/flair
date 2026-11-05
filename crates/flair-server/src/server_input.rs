use super::*;
use flair_core::{
    command::Command,
    key::{InputEvent, Key, KeyInput, MouseAction, MouseButton, MouseInput},
};
use flair_term::{
    encode::{encode_key, encode_mouse, encode_paste, encode_text},
    terminal::MouseMode,
};

impl Server {
    pub(super) fn command(
        &mut self,
        now: Millis,
        id: ClientId,
        command: Command,
        out: &mut Vec<Effect>,
    ) {
        match command {
            Command::Switch(target) => {
                let state = &self.clients[&id];
                let size = state.size;
                let palette = state.palette.clone();
                let env = state.env.clone();
                match self.resolve_target(target, size, &palette, &env, out) {
                    Ok(session) => self.switch_to(now, id, session, out),
                    Err(error) => out.push(Effect::Send(id, ServerMsg::Error(error))),
                }
            }
            Command::LastSession => {
                if let Some(session) = self.clients[&id].view.last_session
                    && self.tree.session(session).is_some()
                {
                    self.switch_to(now, id, session, out);
                }
            }
            Command::Sessionizer => {
                let client = &self.clients[&id];
                let current = self.tree.session(client.view.session).unwrap().name.clone();
                let last = client
                    .view
                    .last_session
                    .and_then(|session| self.tree.session(session))
                    .map(|session| Target::Name(session.name.clone()));
                out.push(Effect::Send(
                    id,
                    ServerMsg::Sessionizer {
                        sessions: self.session_infos(),
                        current,
                        last,
                    },
                ));
            }
            Command::CopySelection => self.copy_selection(id, false, out),
            Command::ScrollLines(_) | Command::ScrollPages(_) | Command::ScrollToBottom => {
                if let Some(pane) = self
                    .client_pane(id)
                    .and_then(|pane| self.panes.get_mut(&pane))
                {
                    match command {
                        Command::ScrollLines(lines) => pane.terminal.scroll(lines),
                        Command::ScrollPages(pages) => pane
                            .terminal
                            .scroll(pages.saturating_mul(i32::from(pane.size.rows))),
                        Command::ScrollToBottom => pane.terminal.scroll_to_bottom(),
                        Command::Switch(_)
                        | Command::LastSession
                        | Command::Sessionizer
                        | Command::CopySelection => unreachable!(),
                    }
                    pane.dirty = true;
                }
            }
        }
    }

    pub(super) fn input(
        &mut self,
        now: Millis,
        id: ClientId,
        input: InputEvent,
        out: &mut Vec<Effect>,
    ) {
        if let InputEvent::Key(key) = &input
            && self.prefix(now, id, key, out)
        {
            return;
        }
        if let InputEvent::Mouse(mouse) = input {
            self.mouse_input(now, id, mouse, out);
            return;
        }
        let Some(pane_id) = self.client_pane(id) else {
            return;
        };
        let pane = self.panes.get_mut(&pane_id).unwrap();
        pane.terminal.clear_selection();
        pane.terminal.scroll_to_bottom();
        pane.dirty = true;
        let modes = pane.terminal.modes();
        let bytes = match input {
            InputEvent::Key(key) => encode_key(&key, &modes),
            InputEvent::Text(text) => Some(encode_text(&text)),
            InputEvent::Paste(text) => Some(encode_paste(&text, &modes)),
            InputEvent::Mouse(_) => unreachable!(),
        };
        if let Some(bytes) = bytes
            && !bytes.is_empty()
        {
            out.push(Effect::WritePty(pane_id, bytes));
        }
    }

    fn prefix(&mut self, now: Millis, id: ClientId, key: &KeyInput, out: &mut Vec<Effect>) -> bool {
        let control_a = key.mods.ctrl
            && !key.mods.alt
            && !key.mods.super_
            && matches!(key.key, Key::Char('a' | 'A'));
        let pending = self.clients[&id]
            .prefix_until
            .is_some_and(|until| now < until);
        if self.clients[&id].prefix_until.is_some() {
            self.clients.get_mut(&id).unwrap().prefix_until = None;
            self.send_title(id, out);
        }
        if pending {
            if control_a {
                return false;
            }
            if key.mods.ctrl || key.mods.alt || key.mods.super_ {
                return true;
            }
            match key.key {
                Key::Char('f') => self.command(now, id, Command::Sessionizer, out),
                Key::Char('l') => self.command(now, id, Command::LastSession, out),
                Key::Char('d') => {
                    self.clients.remove(&id);
                    out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::Detached)));
                }
                Key::Char(_) | Key::Named(_) => {}
            }
            return true;
        }
        if control_a && !key.repeat {
            let until = now + Millis(1000);
            self.clients.get_mut(&id).unwrap().prefix_until = Some(until);
            out.push(Effect::SetTimer(u64::from(id.0), until));
            self.send_title(id, out);
            return true;
        }
        false
    }

    fn mouse_input(&mut self, now: Millis, id: ClientId, mouse: MouseInput, out: &mut Vec<Effect>) {
        let Some(pane_id) = self.client_pane(id) else {
            return;
        };
        let modes = self.panes[&pane_id].terminal.modes();
        if modes.mouse != MouseMode::Off && !mouse.mods.shift {
            if let Some(bytes) = encode_mouse(&mouse, &modes) {
                out.push(Effect::WritePty(pane_id, bytes));
            }
            return;
        }
        match (mouse.button, mouse.action) {
            (MouseButton::Left, MouseAction::Press) => {
                let client = self.clients.get_mut(&id).unwrap();
                let clicks = client
                    .last_click
                    .filter(|(last, point, _)| {
                        now.saturating_sub(*last).0 <= 400 && *point == mouse.point
                    })
                    .map_or(1, |(_, _, clicks)| clicks % 3 + 1);
                client.last_click = Some((now, mouse.point, clicks));
                client.selecting = true;
                let pane = self.panes.get_mut(&pane_id).unwrap();
                pane.terminal.select_start(mouse.point, clicks);
                pane.dirty = true;
            }
            (MouseButton::Left | MouseButton::NoButton, MouseAction::Motion) => {
                if self.clients[&id].selecting {
                    let pane = self.panes.get_mut(&pane_id).unwrap();
                    pane.terminal.select_update(mouse.point);
                    pane.dirty = true;
                }
            }
            (MouseButton::Left, MouseAction::Release) => {
                if self.clients[&id].selecting {
                    self.clients.get_mut(&id).unwrap().selecting = false;
                    let pane = self.panes.get_mut(&pane_id).unwrap();
                    pane.terminal.select_update(mouse.point);
                    pane.dirty = true;
                    self.copy_selection(id, true, out);
                }
            }
            (MouseButton::WheelUp | MouseButton::WheelDown, MouseAction::Press) => {
                let pane = self.panes.get_mut(&pane_id).unwrap();
                pane.terminal
                    .scroll(if mouse.button == MouseButton::WheelUp {
                        3
                    } else {
                        -3
                    });
                pane.dirty = true;
            }
            (
                MouseButton::Middle
                | MouseButton::Right
                | MouseButton::WheelLeft
                | MouseButton::WheelRight,
                _,
            )
            | (
                MouseButton::WheelUp | MouseButton::WheelDown,
                MouseAction::Release | MouseAction::Motion,
            )
            | (MouseButton::NoButton, MouseAction::Press | MouseAction::Release) => {}
        }
    }

    fn copy_selection(&self, id: ClientId, primary: bool, out: &mut Vec<Effect>) {
        if let Some(text) = self
            .client_pane(id)
            .and_then(|pane| self.panes[&pane].terminal.selection_text())
        {
            out.push(Effect::Send(id, ServerMsg::SetClipboard { text, primary }));
        }
    }
}
