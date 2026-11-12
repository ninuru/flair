use super::*;

impl SettingsPage {
    pub(crate) fn set_prompt_scroll_limit(&mut self, limit: usize) {
        self.prompt_editor.set_scroll_limit(limit);
    }

    pub(super) fn prompt_key(&mut self, key: &KeyInput) -> SettingsOutcome {
        if key.key == Key::Named(NamedKey::Tab) {
            self.focus_sections();
            return SettingsOutcome::Pending;
        }
        let action = match key.key {
            Key::Named(NamedKey::Left) if key.mods.shift => Some(PromptAction::Move(-1)),
            Key::Named(NamedKey::Right) if key.mods.shift => Some(PromptAction::Move(1)),
            Key::Named(NamedKey::Left | NamedKey::Up | NamedKey::Right | NamedKey::Down) => {
                let order = self.prompt_editor.ordered();
                let current = self.prompt_editor.selected();
                let index = order.iter().position(|chip| *chip == current).unwrap_or(0);
                let delta = if matches!(key.key, Key::Named(NamedKey::Left | NamedKey::Up)) {
                    -1
                } else {
                    1
                };
                let target = (index as i32 + delta).rem_euclid(order.len() as i32) as usize;
                Some(PromptAction::Select(order[target]))
            }
            Key::Named(NamedKey::Enter) => Some(PromptAction::Save),
            Key::Named(NamedKey::Escape) => {
                if self.prompt_editor.dirty() {
                    Some(PromptAction::Cancel)
                } else {
                    return SettingsOutcome::Close;
                }
            }
            Key::Char(' ') => Some(PromptAction::Toggle),
            Key::Char('c') => Some(PromptAction::Colour),
            Key::Char('a') => Some(PromptAction::Appearance),
            Key::Char('i') => Some(PromptAction::Icon),
            Key::Char('r') => Some(PromptAction::Reset),
            Key::Char('/') => {
                self.focus_sections();
                None
            }
            Key::Char(_) | Key::Named(_) => None,
        };
        action.map_or(SettingsOutcome::Pending, |action| {
            self.prompt_action(action)
        })
    }

    pub(crate) fn is_prompt_editor(&self) -> bool {
        !self.search_focused
            && Section::ALL[self.section] == Section::Prompt
            && self.query.is_empty()
            && self.editor.is_none()
    }

    pub(crate) fn prompt_editor(&self) -> &PromptEditor {
        &self.prompt_editor
    }

    pub(super) fn prompt_action(&mut self, action: PromptAction) -> SettingsOutcome {
        match action {
            PromptAction::Select(_) => {
                self.prompt_editor.act(action);
                SettingsOutcome::Pending
            }
            PromptAction::Save if !self.prompt_editor.dirty() => SettingsOutcome::Pending,
            PromptAction::Save => SettingsOutcome::SavePrompt(self.prompt_editor.changes()),
            PromptAction::Cancel => {
                self.prompt_editor.cancel();
                SettingsOutcome::Revert
            }
            PromptAction::Add(_)
            | PromptAction::Move(_)
            | PromptAction::Toggle
            | PromptAction::Icon
            | PromptAction::Colour
            | PromptAction::Appearance
            | PromptAction::Path
            | PromptAction::Time
            | PromptAction::Date
            | PromptAction::Separator
            | PromptAction::GlobalAppearance
            | PromptAction::GlobalIcons
            | PromptAction::Style
            | PromptAction::Reset => {
                self.prompt_editor.act(action);
                let mut config = self.config.clone();
                config.prompt = self.prompt_editor.draft.clone();
                SettingsOutcome::Preview(Box::new(config))
            }
        }
    }
}
