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

    pub(super) fn copy_command(&mut self, id: ClientId, command: CopyCommand) {
        let Some(pane) = self.copy_pane(id) else {
            return;
        };
        match command {
            CopyCommand::Cancel => self.exit_copy_mode(pane),
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
