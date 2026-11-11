use super::*;
use crate::settings_layout::ChoiceMenu;

impl SettingsPage {
    pub(super) fn activate(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        if self.pending.is_some() {
            return self.leave();
        }
        match SETTINGS[index].kind {
            Kind::Toggle => self.toggle(),
            Kind::Choice(_) => self.open_choice(),
            Kind::Integer { .. }
            | Kind::Float { .. }
            | Kind::Text
            | Kind::Font
            | Kind::Words
            | Kind::Color { .. } => self.edit(),
        }
    }

    pub(crate) fn dismiss_dropdown(&mut self) -> bool {
        self.dropdown.take().is_some()
    }

    pub(super) fn open_choice(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        let options = match SETTINGS[index].kind {
            Kind::Choice(Choices::Themes) => self.themes.clone(),
            Kind::Choice(Choices::Of(names)) => {
                names.iter().map(|name| (*name).to_owned()).collect()
            }
            Kind::Toggle
            | Kind::Integer { .. }
            | Kind::Float { .. }
            | Kind::Text
            | Kind::Font
            | Kind::Words
            | Kind::Color { .. } => return SettingsOutcome::Pending,
        };
        if options.is_empty() {
            return SettingsOutcome::Pending;
        }
        let current = self.shown(index).display();
        let selected = options
            .iter()
            .position(|name| *name == current)
            .unwrap_or(0);
        self.dropdown = Some(Dropdown {
            item: self.item,
            options,
            selected,
        });
        SettingsOutcome::Pending
    }

    pub(super) fn dropdown_sheet(&self) -> Option<ChoiceMenu> {
        let menu = self.dropdown.as_ref()?;
        Some(ChoiceMenu {
            item: menu.item,
            options: menu.options.clone(),
            selected: menu.selected,
        })
    }

    pub(super) fn move_choice(&mut self, delta: i32) {
        if let Some(menu) = &mut self.dropdown {
            menu.selected = menu
                .selected
                .saturating_add_signed(delta as isize)
                .min(menu.options.len().saturating_sub(1));
        }
    }

    pub(super) fn choose(&mut self, selected: usize) -> SettingsOutcome {
        let Some(menu) = &self.dropdown else {
            return SettingsOutcome::Pending;
        };
        let Some(value) = menu.options.get(selected).cloned() else {
            return SettingsOutcome::Pending;
        };
        let Some(Item::Setting(index)) = self.items().get(menu.item).copied() else {
            return SettingsOutcome::Pending;
        };
        self.dropdown = None;
        self.pending = None;
        if Value::Text(value.clone()) == self.current(index) {
            return SettingsOutcome::Revert;
        }
        SettingsOutcome::Save {
            key: SETTINGS[index].key,
            value: Some(Value::Text(value)),
        }
    }

    pub(super) fn dropdown_key(&mut self, key: &KeyInput) -> SettingsOutcome {
        match key.key {
            Key::Named(NamedKey::Escape | NamedKey::Tab) => {
                self.dismiss_dropdown();
            }
            Key::Named(NamedKey::Up) => self.move_choice(-1),
            Key::Named(NamedKey::Down) => self.move_choice(1),
            Key::Named(NamedKey::Home) => self.move_choice(i32::MIN),
            Key::Named(NamedKey::End) => self.move_choice(i32::MAX),
            Key::Named(NamedKey::Enter) => {
                return self
                    .dropdown
                    .as_ref()
                    .map(|menu| menu.selected)
                    .map_or(SettingsOutcome::Pending, |selected| self.choose(selected));
            }
            Key::Char(_) | Key::Named(_) => {}
        }
        SettingsOutcome::Pending
    }
}
