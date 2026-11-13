use flair_core::key::{Key, KeyAction, KeyInput, NamedKey};

use crate::{
    overlay_layout::{Field, Placement, Row, Sheet},
    ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY},
};

pub struct ThemePicker {
    names: Vec<String>,
    current: String,
    query: String,
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeOutcome {
    Pending,
    Preview(String),
    Cancel,
    Save(String),
}

impl ThemePicker {
    pub fn new(names: Vec<String>, current: &str) -> ThemePicker {
        let selected = names.iter().position(|name| name == current).unwrap_or(0);
        ThemePicker {
            names,
            current: current.to_owned(),
            query: String::new(),
            selected,
        }
    }

    pub fn key(&mut self, key: &KeyInput) -> ThemeOutcome {
        if key.action == KeyAction::Release {
            return ThemeOutcome::Pending;
        }
        let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.super_;
        match key.key {
            Key::Named(NamedKey::Escape) => ThemeOutcome::Cancel,
            Key::Named(NamedKey::Enter) => self.selection().map_or(ThemeOutcome::Pending, |name| {
                ThemeOutcome::Save(name.to_owned())
            }),
            Key::Named(NamedKey::Up) => self.step(-1),
            Key::Named(NamedKey::Down) => self.step(1),
            Key::Char('p' | 'P') if key.mods.ctrl => self.step(-1),
            Key::Char('n' | 'N') if key.mods.ctrl => self.step(1),
            Key::Char('u' | 'U') if key.mods.ctrl => {
                let changed = !self.query.is_empty();
                self.query.clear();
                self.filtered(changed)
            }
            Key::Named(NamedKey::Backspace) => {
                let changed = self.query.pop().is_some();
                self.filtered(changed)
            }
            Key::Char(_) if plain => match &key.text {
                Some(text) => self.insert(text),
                None => ThemeOutcome::Pending,
            },
            Key::Char(_) | Key::Named(_) => ThemeOutcome::Pending,
        }
    }

    pub fn insert(&mut self, text: &str) -> ThemeOutcome {
        let before = self.query.len();
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        self.filtered(self.query.len() != before)
    }

    pub fn refresh(&mut self, current: &str) -> ThemeOutcome {
        current.clone_into(&mut self.current);
        self.preview()
    }

    pub fn matches(&self) -> Vec<&str> {
        let query = self.query.to_lowercase();
        self.names
            .iter()
            .map(String::as_str)
            .filter(|name| name.to_lowercase().contains(&query))
            .collect()
    }

    pub fn select(&mut self, index: usize) -> ThemeOutcome {
        if index >= self.matches().len() || index == self.selected {
            return ThemeOutcome::Pending;
        }
        self.selected = index;
        self.preview()
    }

    pub fn sheet(&self) -> Sheet {
        let rows: Vec<Row> = self
            .matches()
            .into_iter()
            .map(|name| Row {
                name: name.to_owned(),
                name_matches: substring(name, &self.query),
                label: (name == self.current).then_some("saved"),
                ..Row::default()
            })
            .collect();
        Sheet {
            strip: None,
            placement: Placement::Docked,
            title: None,
            field: Some(Field {
                text: self.query.clone(),
                cursor: self.query.chars().count(),
                placeholder: "Search themes",
            }),
            count: (rows.len(), self.names.len()),
            rows,
            selected: self.selected,
            keys: vec![
                (ENTER_KEY, "save".to_owned()),
                (MOVE_KEY, "move".to_owned()),
                (ESCAPE_KEY, "revert".to_owned()),
            ],
        }
    }

    fn selection(&self) -> Option<&str> {
        self.matches().get(self.selected).copied()
    }

    pub fn step(&mut self, delta: isize) -> ThemeOutcome {
        let count = self.matches().len();
        let next = self
            .selected
            .saturating_add_signed(delta)
            .min(count.saturating_sub(1));
        if count == 0 || next == self.selected {
            return ThemeOutcome::Pending;
        }
        self.selected = next;
        self.preview()
    }

    fn filtered(&mut self, changed: bool) -> ThemeOutcome {
        if !changed {
            return ThemeOutcome::Pending;
        }
        self.selected = 0;
        self.preview()
    }

    fn preview(&self) -> ThemeOutcome {
        self.selection().map_or(ThemeOutcome::Pending, |name| {
            ThemeOutcome::Preview(name.to_owned())
        })
    }
}

fn substring(name: &str, query: &str) -> Vec<usize> {
    let fold = |text: &str| -> Vec<char> {
        text.chars()
            .map(|c| c.to_lowercase().next().unwrap_or(c))
            .collect()
    };
    let (name, query) = (fold(name), fold(query));
    if query.is_empty() {
        return Vec::new();
    }
    name.windows(query.len())
        .position(|window| window == query.as_slice())
        .map_or_else(Vec::new, |start| (start..start + query.len()).collect())
}

#[cfg(test)]
#[path = "theme_picker_tests.rs"]
mod tests;
