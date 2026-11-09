use super::*;
use flair_core::{
    command::{Command, CopyCommand},
    key::{KeyAction, KeyInput},
};

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
            pane.dirty = true;
        }
    }

    pub(super) fn exit_copy_mode(&mut self, pane: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane)
            && pane.terminal.copy_mode()
        {
            pane.terminal.exit_copy_mode();
            pane.dirty = true;
        }
    }

    pub(super) fn copy_key(
        &mut self,
        now: Millis,
        id: ClientId,
        key: &KeyInput,
        out: &mut Vec<Effect>,
    ) {
        if key.action == KeyAction::Press
            && let Some(command) = self.config.keys.copy_binding(key)
        {
            self.command(now, id, Command::Copy(command), out);
        }
    }

    pub(super) fn copy_command(
        &mut self,
        id: ClientId,
        command: CopyCommand,
        out: &mut Vec<Effect>,
    ) {
        let Some(pane_id) = self.copy_pane(id) else {
            return;
        };
        let pane = self.panes.get_mut(&pane_id).unwrap();
        match command {
            CopyCommand::Cancel => self.exit_copy_mode(pane_id),
            CopyCommand::Move(motion) => pane.terminal.copy_move(motion),
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
            CopyCommand::SearchAgain => pane.terminal.copy_search_again(false),
            CopyCommand::SearchReverse => pane.terminal.copy_search_again(true),
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

#[cfg(test)]
#[path = "server_copy_tests.rs"]
mod tests;
