use super::*;
use flair_core::{command::Command, key::InputEvent};
use flair_term::encode::{encode_key, encode_paste, encode_text_with_modes};

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
            Command::Detach => {
                self.remove_client(id, out);
                out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::Detached)));
            }
            Command::NewWindow => self.new_window(now, id, out),
            Command::NextWindow => self.cycle_window(now, id, true, out),
            Command::PreviousWindow => self.cycle_window(now, id, false, out),
            Command::LastWindow => {
                if let Some(window) = self.clients[&id].view.last_window {
                    self.select_window(now, id, window, out);
                }
            }
            Command::SelectWindow(target) => {
                if let Some(window) = self.window_target(id, target) {
                    self.select_window(now, id, window, out);
                }
            }
            Command::ChooseWindow => out.push(Effect::Send(id, ServerMsg::ChooseWindow)),
            Command::RenameWindow(name) => {
                let window = self.clients[&id].view.window;
                if let Some(name) = name {
                    if name.len() > 255 || name.chars().any(char::is_control) {
                        out.push(Effect::Send(
                            id,
                            ServerMsg::Error(
                                "window name must have at most 255 bytes and no controls".into(),
                            ),
                        ));
                    } else {
                        self.tree.window_mut(window).unwrap().name =
                            (!name.is_empty()).then_some(name);
                    }
                } else {
                    let current = self
                        .tree
                        .window(window)
                        .unwrap()
                        .name
                        .clone()
                        .unwrap_or_default();
                    out.push(Effect::Send(id, ServerMsg::PromptRename { current }));
                }
            }
            Command::CloseWindow => {
                let panes = self
                    .tree
                    .window(self.clients[&id].view.window)
                    .unwrap()
                    .panes
                    .clone();
                for pane in panes {
                    self.destroy_pane(now, pane, out);
                }
            }
            Command::Split(axis) => self.split(now, id, axis, out),
            Command::SelectPane(target) => {
                self.touch(now, id, out);
                self.select_pane(id, target, out);
            }
            Command::ClosePane => {
                if let Some(pane) = self.client_pane(id) {
                    self.destroy_pane(now, pane, out);
                }
            }
            Command::ToggleZoom => {
                let window_id = self.clients[&id].view.window;
                let window = self.tree.window_mut(window_id).unwrap();
                window.zoomed = !window.zoomed;
                self.invalidate_window(window_id, out);
            }
            Command::PasteClipboard => out.push(Effect::Send(id, ServerMsg::PasteClipboard)),
            Command::ReloadConfig => self.reload_config(out),
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
                        | Command::CopySelection
                        | Command::Detach
                        | Command::NewWindow
                        | Command::NextWindow
                        | Command::PreviousWindow
                        | Command::LastWindow
                        | Command::SelectWindow(_)
                        | Command::ChooseWindow
                        | Command::RenameWindow(_)
                        | Command::CloseWindow
                        | Command::Split(_)
                        | Command::SelectPane(_)
                        | Command::ClosePane
                        | Command::ToggleZoom
                        | Command::PasteClipboard
                        | Command::ReloadConfig => unreachable!(),
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
        match input {
            InputEvent::Key(key) => self.key_input(now, id, key, out),
            InputEvent::Ime(composing) => self.set_ime(id, composing, out),
            InputEvent::Text(_) | InputEvent::Paste(_) => {
                self.cancel_prefix(id, out);
                self.forward(id, input, out);
            }
            InputEvent::Mouse(mouse) => self.mouse_input(now, id, mouse, out),
        }
    }

    pub(super) fn forward(&mut self, id: ClientId, input: InputEvent, out: &mut Vec<Effect>) {
        let Some(pane_id) = self.client_pane(id) else {
            return;
        };
        let pane = self.panes.get_mut(&pane_id).unwrap();
        let modes = pane.terminal.modes();
        let bytes = match input {
            InputEvent::Key(key) => encode_key(&key, &modes),
            InputEvent::Text(text) => Some(encode_text_with_modes(&text, &modes)),
            InputEvent::Paste(text) => Some(encode_paste(&text, &modes)),
            InputEvent::Mouse(_) | InputEvent::Ime(_) => unreachable!(),
        };
        if let Some(bytes) = bytes
            && !bytes.is_empty()
        {
            pane.terminal.clear_selection();
            pane.terminal.scroll_to_bottom();
            pane.dirty = true;
            out.push(Effect::WritePty(pane_id, bytes));
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
