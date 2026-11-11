use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

use crate::isles::Chip;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PromptConfig {
    pub style: PromptStyle,
    pub order: PromptOrder,
    pub path: PromptPath,
    pub appearance: PromptAppearance,
    pub time_format: PromptTimeFormat,
    pub date_format: PromptDateFormat,
    pub separator: PromptSeparator,
    pub segments: BTreeMap<Chip, SegmentConfig>,
    pub date: bool,
    pub icons: bool,
    pub cwd: bool,
    pub git_branch: bool,
    pub git_ahead_behind: bool,
    pub git_changes: bool,
    pub exit: bool,
    pub duration_ms: u64,
    pub time: bool,
    pub user: bool,
    pub host: bool,
    pub venv: bool,
    pub docker: bool,
    pub kube: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptStyle {
    Chips,
    Ps1,
}

impl PromptStyle {
    pub const NAMES: [&'static str; 2] = ["chips", "ps1"];

    pub fn name(self) -> &'static str {
        match self {
            PromptStyle::Chips => Self::NAMES[0],
            PromptStyle::Ps1 => Self::NAMES[1],
        }
    }
}

impl PromptConfig {
    pub fn enabled(&self, chip: Chip) -> bool {
        match chip {
            Chip::User => self.user,
            Chip::Host => self.host,
            Chip::Cwd => self.cwd,
            Chip::GitBranch => self.git_branch,
            Chip::GitAheadBehind => self.git_ahead_behind,
            Chip::GitChanges => self.git_changes,
            Chip::Venv => self.venv,
            Chip::Docker => self.docker,
            Chip::Kube => self.kube,
            Chip::Exit => self.exit,
            Chip::Time => self.time,
            Chip::Date => self.date,
        }
    }

    pub fn band(&self) -> u16 {
        match self.style {
            PromptStyle::Chips => 1,
            PromptStyle::Ps1 => 0,
        }
    }
}

impl Default for PromptConfig {
    fn default() -> PromptConfig {
        PromptConfig {
            style: PromptStyle::Chips,
            order: PromptOrder::default(),
            path: PromptPath::Abbreviated,
            appearance: PromptAppearance::Outlined,
            time_format: PromptTimeFormat::TwentyFour,
            date_format: PromptDateFormat::Iso,
            separator: PromptSeparator::None,
            segments: BTreeMap::new(),
            date: false,
            icons: true,
            cwd: true,
            git_branch: true,
            git_ahead_behind: true,
            git_changes: true,
            exit: true,
            duration_ms: 2000,
            time: false,
            user: false,
            host: false,
            venv: false,
            docker: false,
            kube: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptOrder(Vec<Chip>);

impl PromptOrder {
    pub fn from_chips(chips: Vec<Chip>) -> Result<Self, String> {
        if chips.is_empty() {
            return Err("choose at least one prompt segment".to_owned());
        }
        for (index, chip) in chips.iter().enumerate() {
            if chips[..index].contains(chip) {
                return Err(format!("duplicate prompt segment {}", chip.name()));
            }
        }
        Ok(Self(chips))
    }

    pub fn chips(&self) -> &[Chip] {
        &self.0
    }
}

impl Default for PromptOrder {
    fn default() -> Self {
        Self(Chip::ALL.to_vec())
    }
}

impl fmt::Display for PromptOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<_> = self.0.iter().map(|chip| chip.name()).collect();
        f.write_str(&names.join(", "))
    }
}

impl FromStr for PromptOrder {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let chips = text
            .split(',')
            .map(|part| {
                let name = part.trim();
                Chip::from_name(name).ok_or_else(|| format!("unknown prompt segment {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_chips(chips)
    }
}

impl<'de> Deserialize<'de> for PromptOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SegmentConfig {
    pub colour: Option<AnsiColour>,
    pub appearance: Option<PromptAppearance>,
    pub icons: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PromptPath {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "abbreviated")]
    Abbreviated,
    #[serde(rename = "folder")]
    Folder,
}

impl PromptPath {
    pub const NAMES: [&'static str; 3] = ["full", "abbreviated", "folder"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Abbreviated => "abbreviated",
            Self::Folder => "folder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PromptAppearance {
    #[serde(rename = "outlined")]
    Outlined,
    #[serde(rename = "filled")]
    Filled,
    #[serde(rename = "plain")]
    Plain,
}

impl PromptAppearance {
    pub const NAMES: [&'static str; 3] = ["outlined", "filled", "plain"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Outlined => "outlined",
            Self::Filled => "filled",
            Self::Plain => "plain",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PromptTimeFormat {
    #[serde(rename = "24h")]
    TwentyFour,
    #[serde(rename = "12h")]
    Twelve,
}

impl PromptTimeFormat {
    pub const NAMES: [&'static str; 2] = ["24h", "12h"];

    pub fn name(self) -> &'static str {
        match self {
            Self::TwentyFour => "24h",
            Self::Twelve => "12h",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PromptDateFormat {
    #[serde(rename = "iso")]
    Iso,
    #[serde(rename = "short")]
    Short,
    #[serde(rename = "long")]
    Long,
}

impl PromptDateFormat {
    pub const NAMES: [&'static str; 3] = ["iso", "short", "long"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Short => "short",
            Self::Long => "long",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PromptSeparator {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "chevron")]
    Chevron,
    #[serde(rename = "dollar")]
    Dollar,
    #[serde(rename = "percent")]
    Percent,
}

impl PromptSeparator {
    pub const NAMES: [&'static str; 4] = ["none", "chevron", "dollar", "percent"];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Chevron => "chevron",
            Self::Dollar => "dollar",
            Self::Percent => "percent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AnsiColour {
    #[serde(rename = "black")]
    Black,
    #[serde(rename = "red")]
    Red,
    #[serde(rename = "green")]
    Green,
    #[serde(rename = "yellow")]
    Yellow,
    #[serde(rename = "blue")]
    Blue,
    #[serde(rename = "magenta")]
    Magenta,
    #[serde(rename = "cyan")]
    Cyan,
    #[serde(rename = "white")]
    White,
}

impl AnsiColour {
    pub const NAMES: [&'static str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Cyan => "cyan",
            Self::White => "white",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Black => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
        }
    }
}
