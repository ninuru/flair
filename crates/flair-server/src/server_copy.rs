use super::*;
use crate::pane::{Awaiting, CopyKeys};
use flair_core::{
    command::{CopyCommand, CopyMotion},
    key::{Key, KeyAction, KeyInput, NamedKey},
};

const COUNT_LIMIT: u32 = 9999;

impl Server {
    pub(super) fn copy_pane(&self, id: ClientId) -> Option<PaneId> {
        self.client_pane(id).filter(|pane| {
            self.panes
                .get(pane)
                .is_some_and(|pane| pane.terminal.copy_mode())
        })
    }

    pub(super) fn enter_copy_mode(&mut self, id: ClientId) {
        if let Some(pane) = self
            .client_pane(id)
            .and_then(|pane| self.panes.get_mut(&pane))
        {
            pane.terminal.enter_copy_mode();
            pane.wheel_copy = false;
            pane.copy_keys = CopyKeys::default();
            pane.dirty = true;
        }
    }

    pub(super) fn exit_copy_mode(&mut self, pane: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane)
            && pane.terminal.copy_mode()
        {
            pane.terminal.exit_copy_mode();
            pane.copy_keys = CopyKeys::default();
            pane.dirty = true;
        }
    }

    pub(super) fn copy_key(&mut self, id: ClientId, key: &KeyInput, out: &mut Vec<Effect>) {
        if key.action != KeyAction::Press || modifier(key.key) {
            return;
        }
        let Some(pane_id) = self.copy_pane(id) else {
            return;
        };
        let pane = self.panes.get_mut(&pane_id).unwrap();
        let keys = std::mem::take(&mut pane.copy_keys);
        let command = match keys.awaiting {
            Some(Awaiting::Jump(kind)) => typed_char(key).map(|ch| CopyCommand::Jump { kind, ch }),
            Some(Awaiting::G) => self.config.keys.copy_g_binding(key),
            None => {
                if let Some(count) = counted(keys.count, key) {
                    pane.copy_keys.count = Some(count);
                    return;
                }
                match self.config.keys.copy_binding(key) {
                    Some(CopyCommand::Cancel)
                        if keys.count.is_some() && key.key == Key::Named(NamedKey::Escape) =>
                    {
                        None
                    }
                    command => command,
                }
            }
        };
        if let Some(command) = command {
            self.copy_command(id, command, keys.count, out);
        }
    }

    pub(super) fn copy_command(
        &mut self,
        id: ClientId,
        command: CopyCommand,
        count: Option<u16>,
        out: &mut Vec<Effect>,
    ) {
        let Some(pane_id) = self.copy_pane(id) else {
            return;
        };
        let pane = self.panes.get_mut(&pane_id).unwrap();
        let times = count.unwrap_or(1);
        match command {
            CopyCommand::Cancel => self.exit_copy_mode(pane_id),
            CopyCommand::Move(motion) => match (motion, count) {
                (CopyMotion::HistoryTop | CopyMotion::HistoryBottom, Some(line)) => {
                    pane.terminal.copy_goto_line(line);
                }
                (motion, _) => {
                    for _ in 0..times {
                        pane.terminal.copy_move(motion);
                    }
                }
            },
            CopyCommand::Select(kind) => pane.terminal.copy_select(kind),
            CopyCommand::OtherEnd => pane.terminal.copy_other_end(),
            CopyCommand::SearchPrompt { forward } => {
                out.push(Effect::Send(id, ServerMsg::PromptSearch { forward }));
            }
            CopyCommand::Search { pattern, forward } => {
                if let Err(error) = pane.terminal.copy_search(&pattern, forward) {
                    log::warn!("copy-mode search {pattern:?} is not a valid regex: {error}");
                }
            }
            CopyCommand::SearchAgain | CopyCommand::SearchReverse => {
                for _ in 0..times {
                    pane.terminal
                        .copy_search_again(command == CopyCommand::SearchReverse);
                }
            }
            CopyCommand::JumpPrompt(kind) => {
                pane.copy_keys = CopyKeys {
                    count,
                    awaiting: Some(Awaiting::Jump(kind)),
                };
            }
            CopyCommand::GPrefix => {
                pane.copy_keys = CopyKeys {
                    count,
                    awaiting: Some(Awaiting::G),
                };
            }
            CopyCommand::Jump { kind, ch } => pane.terminal.copy_jump(kind, ch, times),
            CopyCommand::JumpAgain => pane.terminal.copy_jump_again(false, times),
            CopyCommand::JumpReverse => pane.terminal.copy_jump_again(true, times),
            CopyCommand::Yank => {
                self.copy_selection(id, false, out);
                self.exit_copy_mode(pane_id);
            }
        }
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.dirty = true;
        }
    }

    pub(super) fn copy_wheel(&mut self, pane: PaneId, up: bool) {
        let pane = self.panes.get_mut(&pane).unwrap();
        if up && !pane.terminal.copy_mode() && !pane.terminal.modes().alt_screen {
            pane.terminal.enter_copy_mode();
            pane.wheel_copy = true;
            pane.copy_keys = CopyKeys::default();
        }
        pane.terminal.scroll(if up { 3 } else { -3 });
        if !up
            && pane.wheel_copy
            && pane.terminal.copy_mode()
            && pane.terminal.display_offset() == 0
        {
            pane.terminal.exit_copy_mode();
        }
        pane.dirty = true;
    }
}

fn modifier(key: Key) -> bool {
    matches!(
        key,
        Key::Named(
            NamedKey::ShiftLeft
                | NamedKey::ShiftRight
                | NamedKey::ControlLeft
                | NamedKey::ControlRight
                | NamedKey::AltLeft
                | NamedKey::AltRight
                | NamedKey::SuperLeft
                | NamedKey::SuperRight
        )
    )
}

fn typed_char(key: &KeyInput) -> Option<char> {
    let text = match (&key.text, key.key) {
        (Some(text), _) => text.clone(),
        (None, Key::Char(c)) if !key.mods.ctrl && !key.mods.alt && !key.mods.super_ => c.into(),
        (None, Key::Char(_) | Key::Named(_)) => return None,
    };
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(c),
        (Some(_) | None, _) => None,
    }
}

fn counted(count: Option<u16>, key: &KeyInput) -> Option<u16> {
    let digit = typed_char(key)?.to_digit(10)?;
    if count.is_none() && digit == 0 {
        return None;
    }
    let value = (u32::from(count.unwrap_or(0)) * 10 + digit).min(COUNT_LIMIT);
    u16::try_from(value).ok()
}

#[cfg(test)]
#[path = "server_copy_tests.rs"]
mod tests;
