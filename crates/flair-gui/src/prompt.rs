use flair_core::{
    command::{Command, CopyCommand, WindowRef},
    key::{Key, KeyAction, KeyInput, NamedKey},
    status::{StatusLine, StatusWindow},
};

use crate::{
    cursor::TextCursor,
    overlay_layout::{Placement, Row, Sheet},
    process_icon::process_icon,
    ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY},
};

const NAME_LIMIT: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    Choose {
        windows: Vec<StatusWindow>,
        selected: usize,
    },
    Rename {
        field: TextCursor,
    },
    Search {
        field: TextCursor,
        forward: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Invalid,
    Close,
    Run(Command),
}

impl Prompt {
    pub fn choose(status: &StatusLine) -> Prompt {
        Prompt::Choose {
            windows: status.windows.clone(),
            selected: status
                .windows
                .iter()
                .position(|window| window.active)
                .unwrap_or(0),
        }
    }

    pub fn rename(current: String) -> Prompt {
        Prompt::Rename {
            field: TextCursor::new(&current).with_limit(NAME_LIMIT),
        }
    }

    pub fn search(forward: bool) -> Prompt {
        Prompt::Search {
            field: TextCursor::new("").with_limit(NAME_LIMIT),
            forward,
        }
    }

    pub fn insert(&mut self, input: &str) {
        match self {
            Prompt::Choose { .. } => {}
            Prompt::Rename { field } | Prompt::Search { field, .. } => field.insert(input),
        }
    }

    pub fn key(&mut self, key: &KeyInput) -> Outcome {
        if key.action == KeyAction::Release {
            return Outcome::Pending;
        }
        let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.super_;
        match self {
            Prompt::Choose { windows, selected } => match key.key {
                Key::Named(NamedKey::Escape) | Key::Char('q') if plain => Outcome::Close,
                Key::Named(NamedKey::Enter) => {
                    windows.get(*selected).map_or(Outcome::Close, |window| {
                        Outcome::Run(Command::SelectWindow(WindowRef::Id(window.id)))
                    })
                }
                Key::Named(NamedKey::Up) | Key::Char('k') if plain => {
                    *selected = selected.saturating_sub(1);
                    Outcome::Pending
                }
                Key::Named(NamedKey::Down) | Key::Char('j') if plain => {
                    *selected = (*selected + 1).min(windows.len().saturating_sub(1));
                    Outcome::Pending
                }
                Key::Char(c @ '1'..='9') if plain => Outcome::Run(Command::SelectWindow(
                    WindowRef::Index(c.to_digit(10).map_or(1, |n| n as u16)),
                )),
                Key::Char(_) | Key::Named(_) => Outcome::Pending,
            },
            Prompt::Rename { field } if key.key == Key::Named(NamedKey::Enter) => {
                if field.is_empty() {
                    Outcome::Invalid
                } else {
                    Outcome::Run(Command::RenameWindow(Some(field.text().to_owned())))
                }
            }
            Prompt::Search { field, forward } if key.key == Key::Named(NamedKey::Enter) => {
                Outcome::Run(Command::Copy(CopyCommand::Search {
                    pattern: field.text().to_owned(),
                    forward: *forward,
                }))
            }
            Prompt::Rename { field } | Prompt::Search { field, .. } => match key.key {
                Key::Named(NamedKey::Escape) => Outcome::Close,
                Key::Char(_) | Key::Named(_) => {
                    field.edit(key);
                    Outcome::Pending
                }
            },
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        match self {
            Prompt::Choose { windows, selected } => {
                *selected = selected
                    .saturating_add_signed(delta)
                    .min(windows.len().saturating_sub(1));
            }
            Prompt::Rename { .. } | Prompt::Search { .. } => {}
        }
    }

    pub fn sheet(&self) -> Option<Sheet> {
        match self {
            Prompt::Choose { windows, selected } => Some(Sheet {
                strip: None,
                placement: Placement::Centred,
                title: Some("Windows"),
                field: None,
                count: (windows.len(), windows.len()),
                rows: windows
                    .iter()
                    .map(|window| Row {
                        number: Some(window.index),
                        icon: process_icon(&window.program),
                        name: window.name.chars().filter(|c| !c.is_control()).collect(),
                        label: if window.active {
                            Some("active")
                        } else if window.last {
                            Some("last")
                        } else {
                            None
                        },
                        ..Row::default()
                    })
                    .collect(),
                selected: *selected,
                keys: vec![
                    (ENTER_KEY, "select".to_owned()),
                    (MOVE_KEY, "move".to_owned()),
                    ("1-9", "jump".to_owned()),
                    (ESCAPE_KEY, "cancel".to_owned()),
                ],
            }),
            Prompt::Rename { .. } | Prompt::Search { .. } => None,
        }
    }

    pub fn bar_text(&self) -> Option<String> {
        Some(format!("{}{}", self.field_prefix()?, self.field_text()?))
    }

    pub fn field_prefix(&self) -> Option<&str> {
        match self {
            Prompt::Choose { .. } => None,
            Prompt::Rename { .. } => Some("(rename-window) "),
            Prompt::Search { forward: true, .. } => Some("/"),
            Prompt::Search { forward: false, .. } => Some("?"),
        }
    }

    pub fn field_text(&self) -> Option<&str> {
        self.field().map(TextCursor::text)
    }

    pub fn cursor(&self) -> Option<usize> {
        self.field().map(TextCursor::cursor)
    }

    fn field(&self) -> Option<&TextCursor> {
        match self {
            Prompt::Choose { .. } => None,
            Prompt::Rename { field } | Prompt::Search { field, .. } => Some(field),
        }
    }
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
