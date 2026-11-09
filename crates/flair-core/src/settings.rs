use toml_edit::Array;

use crate::{
    config::Config,
    config_edit::{contains, document},
    theme::{Rgb, Rgba, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Font,
    Window,
    Terminal,
    Keys,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Appearance,
        Section::Font,
        Section::Window,
        Section::Terminal,
        Section::Keys,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Appearance => "Appearance",
            Section::Font => "Font",
            Section::Window => "Window",
            Section::Terminal => "Terminal",
            Section::Keys => "Keys",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Default,
    File,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Default => "default",
            Source::File => "flair.toml",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choices {
    Themes,
    Fixed(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Toggle,
    Integer { step: i64, min: i64, max: i64 },
    Float { step: f64, min: f64 },
    Choice(Choices),
    Text,
    Font,
    Words,
    Color { alpha: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Words(Vec<String>),
}

impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Bool(true) => "on".to_owned(),
            Value::Bool(false) => "off".to_owned(),
            Value::Integer(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::Text(text) => text.clone(),
            Value::Words(words) => words.join(" "),
        }
    }

    pub(crate) fn to_toml(&self) -> toml_edit::Value {
        match self {
            Value::Bool(value) => (*value).into(),
            Value::Integer(value) => (*value).into(),
            Value::Float(value) => (*value).into(),
            Value::Text(text) => text.as_str().into(),
            Value::Words(words) => words.iter().map(String::as_str).collect::<Array>().into(),
        }
    }
}

pub struct Setting {
    pub key: &'static str,
    pub title: &'static str,
    pub section: Section,
    pub kind: Kind,
    pub description: &'static str,
    read: fn(&Config, &Theme) -> Value,
}

impl Setting {
    pub fn value(&self, config: &Config, theme: &Theme) -> Value {
        (self.read)(config, theme)
    }

    pub fn parse(&self, text: &str) -> Result<Option<Value>, String> {
        let trimmed = text.trim();
        match self.kind {
            Kind::Toggle | Kind::Choice(_) => Err("use space or ← →".to_owned()),
            Kind::Integer { min, max, .. } => trimmed
                .parse::<i64>()
                .ok()
                .filter(|value| (min..=max).contains(value))
                .map(|value| Some(Value::Integer(value)))
                .ok_or_else(|| {
                    if max == i64::MAX {
                        format!("enter a whole number of at least {min}")
                    } else {
                        format!("enter a whole number from {min} to {max}")
                    }
                }),
            Kind::Float { .. } => trimmed
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value > 0.0)
                .map(|value| Some(Value::Float(value)))
                .ok_or_else(|| "enter a number greater than 0".to_owned()),
            Kind::Text | Kind::Font => Ok(Some(Value::Text(trimmed.to_owned()))),
            Kind::Words => Ok(Some(Value::Words(
                text.split_whitespace().map(str::to_owned).collect(),
            ))),
            Kind::Color { .. } if trimmed.is_empty() => Ok(None),
            Kind::Color { alpha: false } => Rgb::from_hex(trimmed)
                .map(|color| Some(Value::Text(color.hex())))
                .map_err(|error| error.to_string()),
            Kind::Color { alpha: true } => Rgba::from_hex(trimmed)
                .map(|color| Some(Value::Text(color.hex())))
                .map_err(|error| error.to_string()),
        }
    }

    pub fn step(&self, current: &Value, delta: i32, themes: &[String]) -> Option<Value> {
        let next = match self.kind {
            Kind::Toggle => {
                let Value::Bool(on) = current else {
                    return None;
                };
                Value::Bool(!on)
            }
            Kind::Integer { step, min, max } => {
                let Value::Integer(value) = current else {
                    return None;
                };
                let moved = value.saturating_add(step.saturating_mul(i64::from(delta)));
                Value::Integer(moved.clamp(min, max))
            }
            Kind::Float { step, min } => {
                let Value::Float(value) = current else {
                    return None;
                };
                let grid = ((value + f64::from(delta) * step) / step).round() * step;
                Value::Float(((grid * 1e6).round() / 1e6).max(min))
            }
            Kind::Choice(choices) => {
                let Value::Text(name) = current else {
                    return None;
                };
                let next = match choices {
                    Choices::Themes => cycle(themes, name, delta),
                    Choices::Fixed(names) => cycle(names, name, delta),
                };
                Value::Text(next?)
            }
            Kind::Text | Kind::Font | Kind::Words | Kind::Color { .. } => return None,
        };
        (next != *current).then_some(next)
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        [self.title, self.key, self.description, self.section.title()]
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
    }
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "theme",
        title: "Theme",
        section: Section::Appearance,
        kind: Kind::Choice(Choices::Themes),
        description: "Colour theme for the terminal and the chrome.",
        read: |config, _| Value::Text(config.theme.clone()),
    },
    Setting {
        key: "colors.background",
        title: "Background",
        section: Section::Appearance,
        kind: Kind::Color { alpha: false },
        description: "Overrides the theme background.",
        read: |_, theme| Value::Text(theme.background.hex()),
    },
    Setting {
        key: "colors.foreground",
        title: "Foreground",
        section: Section::Appearance,
        kind: Kind::Color { alpha: false },
        description: "Overrides the theme text colour.",
        read: |_, theme| Value::Text(theme.foreground.hex()),
    },
    Setting {
        key: "colors.cursor",
        title: "Cursor colour",
        section: Section::Appearance,
        kind: Kind::Color { alpha: false },
        description: "Overrides the cursor colour.",
        read: |_, theme| Value::Text(theme.cursor.hex()),
    },
    Setting {
        key: "colors.accent",
        title: "Accent",
        section: Section::Appearance,
        kind: Kind::Color { alpha: false },
        description: "Overrides the accent of the caret and the active pane.",
        read: |_, theme| Value::Text(theme.accent.hex()),
    },
    Setting {
        key: "colors.selection",
        title: "Selection",
        section: Section::Appearance,
        kind: Kind::Color { alpha: true },
        description: "Selection colour with alpha, as #rrggbbaa.",
        read: |_, theme| Value::Text(theme.selection.hex()),
    },
    Setting {
        key: "font.family",
        title: "Family",
        section: Section::Font,
        kind: Kind::Font,
        description: "Terminal font family.",
        read: |config, _| Value::Text(config.font.family.clone()),
    },
    Setting {
        key: "font.size",
        title: "Size",
        section: Section::Font,
        kind: Kind::Float {
            step: 0.5,
            min: 0.5,
        },
        description: "Terminal font size in points.",
        read: |config, _| Value::Float(decimal(config.font.size)),
    },
    Setting {
        key: "font.line_height",
        title: "Line height",
        section: Section::Font,
        kind: Kind::Float {
            step: 0.05,
            min: 0.05,
        },
        description: "Line height as a multiple of the font size.",
        read: |config, _| Value::Float(decimal(config.font.line_height)),
    },
    Setting {
        key: "font.ui_family",
        title: "UI family",
        section: Section::Font,
        kind: Kind::Font,
        description: "Font family of the bar and panels.",
        read: |config, _| Value::Text(config.font.ui_family.clone()),
    },
    Setting {
        key: "font.ui_size",
        title: "UI size",
        section: Section::Font,
        kind: Kind::Float {
            step: 0.5,
            min: 0.5,
        },
        description: "Font size of the bar and panels.",
        read: |config, _| Value::Float(decimal(config.font.ui_size)),
    },
    Setting {
        key: "window.padding_x",
        title: "Padding X",
        section: Section::Window,
        kind: Kind::Integer {
            step: 1,
            min: 0,
            max: 65535,
        },
        description: "Space left and right of the terminal grid.",
        read: |config, _| Value::Integer(i64::from(config.window.padding_x)),
    },
    Setting {
        key: "window.padding_y",
        title: "Padding Y",
        section: Section::Window,
        kind: Kind::Integer {
            step: 1,
            min: 0,
            max: 65535,
        },
        description: "Space above and below the terminal grid.",
        read: |config, _| Value::Integer(i64::from(config.window.padding_y)),
    },
    Setting {
        key: "shell.program",
        title: "Shell",
        section: Section::Terminal,
        kind: Kind::Text,
        description: "Program started in new panes.",
        read: |config, _| Value::Text(config.shell.program.clone()),
    },
    Setting {
        key: "shell.args",
        title: "Shell arguments",
        section: Section::Terminal,
        kind: Kind::Words,
        description: "Arguments of the shell, separated by spaces.",
        read: |config, _| Value::Words(config.shell.args.clone()),
    },
    Setting {
        key: "scrollback.lines",
        title: "Scrollback",
        section: Section::Terminal,
        kind: Kind::Integer {
            step: 1000,
            min: 0,
            max: i64::MAX,
        },
        description: "History lines kept by new panes.",
        read: |config, _| {
            Value::Integer(i64::try_from(config.scrollback.lines).unwrap_or(i64::MAX))
        },
    },
    Setting {
        key: "keys.enabled",
        title: "Bindings",
        section: Section::Keys,
        kind: Kind::Toggle,
        description: "Prefix and root bindings. Off also disables prefix S.",
        read: |config, _| Value::Bool(config.keys.enabled),
    },
    Setting {
        key: "keys.key",
        title: "Prefix key",
        section: Section::Keys,
        kind: Kind::Text,
        description: "Chord that starts a prefix binding, like C-a.",
        read: |config, _| Value::Text(config.keys.key.clone()),
    },
    Setting {
        key: "keys.timeout_ms",
        title: "Prefix timeout",
        section: Section::Keys,
        kind: Kind::Integer {
            step: 100,
            min: 0,
            max: i64::MAX,
        },
        description: "Milliseconds to wait for a key after the prefix.",
        read: |config, _| Value::Integer(i64::try_from(config.keys.timeout_ms).unwrap_or(i64::MAX)),
    },
];

pub fn position(key: &str) -> Option<usize> {
    SETTINGS.iter().position(|setting| setting.key == key)
}

pub fn sources(text: &str) -> Vec<Source> {
    let doc = document(text).ok();
    SETTINGS
        .iter()
        .map(|setting| {
            if doc.as_ref().is_some_and(|doc| contains(doc, setting.key)) {
                Source::File
            } else {
                Source::Default
            }
        })
        .collect()
}

fn decimal(value: f32) -> f64 {
    value.to_string().parse().unwrap_or(f64::from(value))
}

fn cycle(names: &[impl AsRef<str>], current: &str, delta: i32) -> Option<String> {
    let len = i64::try_from(names.len()).ok().filter(|len| *len > 0)?;
    let start = match names.iter().position(|name| name.as_ref() == current) {
        Some(index) => i64::try_from(index).ok()?,
        None if delta < 0 => len,
        None => -1,
    };
    let index = usize::try_from((start + i64::from(delta)).rem_euclid(len)).ok()?;
    Some(names[index].as_ref().to_owned())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
