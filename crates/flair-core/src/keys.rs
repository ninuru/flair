use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::{
    command::{Command, CopyCommand, CopyMotion, PaneRef, SelectionKind, WindowRef},
    geom::{Axis, Direction},
    key::{Key, KeyInput, Mods, NamedKey},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeysConfig {
    pub enabled: bool,
    pub key: String,
    pub timeout_ms: u64,
    pub root: BTreeMap<String, String>,
    pub prefix: BTreeMap<String, String>,
    #[serde(rename = "copy-mode")]
    pub copy_mode: BTreeMap<String, String>,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key: "C-a".into(),
            timeout_ms: 1500,
            root: BTreeMap::new(),
            prefix: BTreeMap::new(),
            copy_mode: BTreeMap::new(),
        }
    }
}

impl KeysConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.key
            .parse::<KeyChord>()
            .map_err(|error| format!("keys.key: {error}"))?;
        for (name, table, copy) in [
            ("root", &self.root, false),
            ("prefix", &self.prefix, false),
            ("copy-mode", &self.copy_mode, true),
        ] {
            let mut chords = HashSet::new();
            for (key, command) in table {
                let chord = key
                    .parse::<KeyChord>()
                    .map_err(|error| format!("keys.{name}.{key}: {error}"))?;
                if !chords.insert(chord) {
                    return Err(format!("keys.{name}.{key}: duplicate normalized key"));
                }
                let known = if copy {
                    parse_copy_command(command).is_some()
                } else {
                    parse_command(command).is_some()
                };
                if command != "none" && !known {
                    return Err(format!("keys.{name}.{key}: unknown command {command:?}"));
                }
            }
        }
        Ok(())
    }
}

pub fn parse_command(command: &str) -> Option<Command> {
    Some(match command {
        "new-window" => Command::NewWindow,
        "rename-window" => Command::RenameWindow(None),
        "next-window" => Command::NextWindow,
        "previous-window" => Command::PreviousWindow,
        "choose-window" => Command::ChooseWindow,
        "detach" => Command::Detach,
        "close-pane" => Command::ClosePane,
        "close-window" => Command::CloseWindow,
        "toggle-zoom" => Command::ToggleZoom,
        "last-window" => Command::LastWindow,
        "last-session" => Command::LastSession,
        "sessionizer" => Command::Sessionizer,
        "choose-session" => Command::ChooseSession,
        "reload-config" => Command::ReloadConfig,
        "select-pane-left" => Command::SelectPane(PaneRef::Direction(Direction::Left)),
        "select-pane-down" => Command::SelectPane(PaneRef::Direction(Direction::Down)),
        "select-pane-up" => Command::SelectPane(PaneRef::Direction(Direction::Up)),
        "select-pane-right" => Command::SelectPane(PaneRef::Direction(Direction::Right)),
        "split-horizontal" => Command::Split(Axis::Horizontal),
        "split-vertical" => Command::Split(Axis::Vertical),
        "copy-selection" => Command::CopySelection,
        "paste-clipboard" => Command::PasteClipboard,
        "scroll-page-up" => Command::ScrollPages(1),
        "scroll-page-down" => Command::ScrollPages(-1),
        "copy-mode" => Command::CopyMode,
        _ => {
            let number = command.strip_prefix("select-window-")?.parse().ok()?;
            if !(1..=9).contains(&number) {
                return None;
            }
            Command::SelectWindow(WindowRef::Index(number))
        }
    })
}

pub fn parse_copy_command(command: &str) -> Option<CopyCommand> {
    Some(match command {
        "cancel" => CopyCommand::Cancel,
        "cursor-left" => CopyCommand::Move(CopyMotion::Left),
        "cursor-down" => CopyCommand::Move(CopyMotion::Down),
        "cursor-up" => CopyCommand::Move(CopyMotion::Up),
        "cursor-right" => CopyCommand::Move(CopyMotion::Right),
        "next-word" => CopyCommand::Move(CopyMotion::NextWord),
        "previous-word" => CopyCommand::Move(CopyMotion::PreviousWord),
        "next-word-end" => CopyCommand::Move(CopyMotion::NextWordEnd),
        "next-space" => CopyCommand::Move(CopyMotion::NextSpace),
        "previous-space" => CopyCommand::Move(CopyMotion::PreviousSpace),
        "next-space-end" => CopyCommand::Move(CopyMotion::NextSpaceEnd),
        "start-of-line" => CopyCommand::Move(CopyMotion::StartOfLine),
        "back-to-indentation" => CopyCommand::Move(CopyMotion::BackToIndentation),
        "end-of-line" => CopyCommand::Move(CopyMotion::EndOfLine),
        "history-top" => CopyCommand::Move(CopyMotion::HistoryTop),
        "history-bottom" => CopyCommand::Move(CopyMotion::HistoryBottom),
        "halfpage-up" => CopyCommand::Move(CopyMotion::HalfPageUp),
        "halfpage-down" => CopyCommand::Move(CopyMotion::HalfPageDown),
        "top-line" => CopyCommand::Move(CopyMotion::TopLine),
        "middle-line" => CopyCommand::Move(CopyMotion::MiddleLine),
        "bottom-line" => CopyCommand::Move(CopyMotion::BottomLine),
        "begin-selection" => CopyCommand::Select(SelectionKind::Simple),
        "select-line" => CopyCommand::Select(SelectionKind::Lines),
        "rectangle-toggle" => CopyCommand::Select(SelectionKind::Block),
        "other-end" => CopyCommand::OtherEnd,
        "search-forward" => CopyCommand::SearchPrompt { forward: true },
        "search-backward" => CopyCommand::SearchPrompt { forward: false },
        "search-again" => CopyCommand::SearchAgain,
        "search-reverse" => CopyCommand::SearchReverse,
        _ => return None,
    })
}

impl KeysConfig {
    pub fn binding(&self, input: &KeyInput, prefix: bool) -> Option<Command> {
        let (overrides, defaults) = if prefix {
            (&self.prefix, PREFIX_DEFAULTS)
        } else {
            (&self.root, ROOT_DEFAULTS)
        };
        lookup(overrides, defaults, input, parse_command)
    }

    pub fn copy_binding(&self, input: &KeyInput) -> Option<CopyCommand> {
        lookup(&self.copy_mode, COPY_DEFAULTS, input, parse_copy_command)
    }
}

fn lookup<T>(
    overrides: &BTreeMap<String, String>,
    defaults: &[(&str, &str)],
    input: &KeyInput,
    parse: fn(&str) -> Option<T>,
) -> Option<T> {
    for (key, command) in overrides {
        if key
            .parse::<KeyChord>()
            .is_ok_and(|chord| chord.matches(input))
        {
            return parse(command);
        }
    }
    defaults.iter().find_map(|(key, command)| {
        key.parse::<KeyChord>()
            .ok()
            .filter(|chord| chord.matches(input))
            .and_then(|_| parse(command))
    })
}

const PREFIX_DEFAULTS: &[(&str, &str)] = &[
    ("c", "new-window"),
    (",", "rename-window"),
    ("n", "next-window"),
    ("p", "previous-window"),
    ("w", "choose-window"),
    ("d", "detach"),
    ("x", "close-pane"),
    ("&", "close-window"),
    ("\"", "split-vertical"),
    ("%", "split-horizontal"),
    ("z", "toggle-zoom"),
    ("^", "last-window"),
    ("l", "last-session"),
    ("f", "sessionizer"),
    ("s", "choose-session"),
    ("r", "reload-config"),
    ("[", "copy-mode"),
    ("h", "select-pane-left"),
    ("j", "select-pane-down"),
    ("k", "select-pane-up"),
    ("1", "select-window-1"),
    ("2", "select-window-2"),
    ("3", "select-window-3"),
    ("4", "select-window-4"),
    ("5", "select-window-5"),
    ("6", "select-window-6"),
    ("7", "select-window-7"),
    ("8", "select-window-8"),
    ("9", "select-window-9"),
];

const COPY_DEFAULTS: &[(&str, &str)] = &[
    ("q", "cancel"),
    ("Escape", "cancel"),
    ("h", "cursor-left"),
    ("j", "cursor-down"),
    ("k", "cursor-up"),
    ("l", "cursor-right"),
    ("Left", "cursor-left"),
    ("Down", "cursor-down"),
    ("Up", "cursor-up"),
    ("Right", "cursor-right"),
    ("w", "next-word"),
    ("b", "previous-word"),
    ("e", "next-word-end"),
    ("W", "next-space"),
    ("B", "previous-space"),
    ("E", "next-space-end"),
    ("0", "start-of-line"),
    ("^", "back-to-indentation"),
    ("$", "end-of-line"),
    ("g", "history-top"),
    ("G", "history-bottom"),
    ("C-u", "halfpage-up"),
    ("C-d", "halfpage-down"),
    ("H", "top-line"),
    ("M", "middle-line"),
    ("L", "bottom-line"),
    ("v", "begin-selection"),
    ("V", "select-line"),
    ("C-v", "rectangle-toggle"),
    ("o", "other-end"),
    ("/", "search-forward"),
    ("?", "search-backward"),
    ("n", "search-again"),
    ("N", "search-reverse"),
];

const ROOT_DEFAULTS: &[(&str, &str)] = &[
    ("C-S-c", "copy-selection"),
    ("C-S-v", "paste-clipboard"),
    ("S-PageUp", "scroll-page-up"),
    ("S-PageDown", "scroll-page-down"),
    ("C-S-Enter", "split-horizontal"),
    ("A-S-Enter", "split-vertical"),
    ("A-h", "select-pane-left"),
    ("A-j", "select-pane-down"),
    ("A-k", "select-pane-up"),
    ("A-l", "select-pane-right"),
    ("A-1", "select-window-1"),
    ("A-2", "select-window-2"),
    ("A-3", "select-window-3"),
    ("A-4", "select-window-4"),
    ("A-5", "select-window-5"),
    ("A-6", "select-window-6"),
    ("A-7", "select-window-7"),
    ("A-8", "select-window-8"),
    ("A-9", "select-window-9"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    key: Key,
    mods: Mods,
}

impl KeyChord {
    pub fn matches(self, input: &KeyInput) -> bool {
        self == Self::normalized(input.key, input.mods)
    }

    fn normalized(key: Key, mut mods: Mods) -> Self {
        let key = match key {
            Key::Char(c) if c.is_ascii_punctuation() => {
                mods.shift = false;
                Key::Char(c)
            }
            Key::Char(c) if c.is_ascii_alphabetic() => Key::Char(c.to_ascii_lowercase()),
            Key::Char(_) | Key::Named(_) => key,
        };
        Self { key, mods }
    }
}

impl FromStr for KeyChord {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut rest = value;
        let mut mods = Mods::default();
        while let Some((modifier, tail)) = rest.split_once('-') {
            let slot = match modifier.to_ascii_lowercase().as_str() {
                "c" | "ctrl" | "control" => &mut mods.ctrl,
                "a" | "alt" | "m" | "meta" => &mut mods.alt,
                "s" | "shift" => &mut mods.shift,
                "super" | "logo" => &mut mods.super_,
                _ => break,
            };
            if *slot {
                return Err(format!("duplicate modifier in key {value:?}"));
            }
            *slot = true;
            rest = tail;
        }
        let key = match rest.to_ascii_lowercase().as_str() {
            "enter" | "return" => Key::Named(NamedKey::Enter),
            "tab" => Key::Named(NamedKey::Tab),
            "backspace" | "bspace" => Key::Named(NamedKey::Backspace),
            "escape" | "esc" => Key::Named(NamedKey::Escape),
            "up" => Key::Named(NamedKey::Up),
            "down" => Key::Named(NamedKey::Down),
            "left" => Key::Named(NamedKey::Left),
            "right" => Key::Named(NamedKey::Right),
            "home" => Key::Named(NamedKey::Home),
            "end" => Key::Named(NamedKey::End),
            "pageup" | "pgup" => Key::Named(NamedKey::PageUp),
            "pagedown" | "pgdn" => Key::Named(NamedKey::PageDown),
            "insert" => Key::Named(NamedKey::Insert),
            "delete" => Key::Named(NamedKey::Delete),
            "space" => Key::Char(' '),
            _ => {
                if let Some(number) = rest.strip_prefix('F').or_else(|| rest.strip_prefix('f'))
                    && let Ok(number @ 1..=35) = number.parse()
                {
                    Key::Named(NamedKey::F(number))
                } else {
                    let mut chars = rest.chars();
                    let c = chars.next().ok_or_else(|| format!("empty key {value:?}"))?;
                    if chars.next().is_some() {
                        return Err(format!("unknown key {value:?}"));
                    }
                    if c.is_ascii_uppercase() {
                        mods.shift = true;
                    }
                    Key::Char(c)
                }
            }
        };
        Ok(Self::normalized(key, mods))
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
