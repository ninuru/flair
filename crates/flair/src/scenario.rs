use flair_core::{
    geom::GridSize,
    key::{Key, KeyAction, KeyInput, Locks, Mods, NamedKey},
    status::STATUS_ROWS,
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scenario {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub cols: u16,
    pub rows: u16,
    pub steps: Vec<Step>,
}
impl Default for Scenario {
    fn default() -> Self {
        Self {
            program: "/bin/sh".into(),
            args: vec![],
            cwd: None,
            env: BTreeMap::new(),
            cols: 80,
            rows: 24,
            steps: vec![],
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Wait {
        text: String,
    },
    Key {
        key: String,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        alt: bool,
    },
    Text {
        text: String,
    },
    Paste {
        text: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Scroll {
        lines: i32,
    },
    Cursor {
        col: u16,
        row: u16,
    },
}
impl Scenario {
    pub fn parse(text: &str) -> Result<Self, String> {
        let scenario: Self = toml::from_str(text).map_err(|e| format!("invalid scenario: {e}"))?;
        client_size(scenario.cols, scenario.rows)?;
        if scenario.program.is_empty() {
            return Err("scenario program is empty".into());
        }
        for step in &scenario.steps {
            match step {
                Step::Resize { cols, rows } => {
                    client_size(*cols, *rows)?;
                }
                Step::Key { key, .. } => {
                    parse_key(key)?;
                }
                Step::Wait { .. }
                | Step::Text { .. }
                | Step::Paste { .. }
                | Step::Scroll { .. }
                | Step::Cursor { .. } => {}
            }
        }
        Ok(scenario)
    }
}
pub fn client_size(cols: u16, rows: u16) -> Result<GridSize, String> {
    let full_rows = rows
        .checked_add(STATUS_ROWS)
        .ok_or("grid is too tall to include the status row")?;
    if cols == 0 || rows == 0 || u32::from(cols) * u32::from(full_rows) > 1_000_000 {
        return Err("grid must be nonzero and contain at most 1000000 cells".into());
    }
    Ok(GridSize {
        cols,
        rows: full_rows,
    })
}
pub fn parse_key(key: &str) -> Result<Key, String> {
    let named = match key {
        "Enter" => Some(NamedKey::Enter),
        "Tab" => Some(NamedKey::Tab),
        "Backspace" => Some(NamedKey::Backspace),
        "Escape" => Some(NamedKey::Escape),
        "Up" => Some(NamedKey::Up),
        "Down" => Some(NamedKey::Down),
        "Left" => Some(NamedKey::Left),
        "Right" => Some(NamedKey::Right),
        "Home" => Some(NamedKey::Home),
        "End" => Some(NamedKey::End),
        "PageUp" => Some(NamedKey::PageUp),
        "PageDown" => Some(NamedKey::PageDown),
        "Insert" => Some(NamedKey::Insert),
        "Delete" => Some(NamedKey::Delete),
        _ => None,
    };
    if let Some(named) = named {
        return Ok(Key::Named(named));
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(Key::Char(c)),
        _ => Err(format!("unknown scenario key {key:?}")),
    }
}
pub fn key_input(key: &str, ctrl: bool, shift: bool, alt: bool) -> Result<KeyInput, String> {
    let key = parse_key(key)?;
    Ok(KeyInput {
        key,
        mods: Mods {
            ctrl,
            shift,
            alt,
            super_: false,
        },
        text: match key {
            Key::Char(c) => Some(c.to_string()),
            Key::Named(_) => None,
        },
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    })
}

#[cfg(test)]
#[path = "scenario_tests.rs"]
mod tests;
