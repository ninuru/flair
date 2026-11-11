use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, de};

use crate::{
    keys::KeysConfig,
    theme::{BUILTIN_THEMES, Rgb, Rgba, Theme},
};

pub use crate::prompt_config::{
    AnsiColour, PromptAppearance, PromptConfig, PromptDateFormat, PromptOrder, PromptPath,
    PromptSeparator, PromptStyle, PromptTimeFormat, SegmentConfig,
};

const MAX_THEME_NAME: usize = 64;
const SELECTION_ALPHA: u8 = 0x40;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: String,
    pub keys: KeysConfig,
    pub shell: ShellConfig,
    #[serde(deserialize_with = "env_over_defaults")]
    pub env: BTreeMap<String, String>,
    pub font: FontConfig,
    pub window: WindowConfig,
    pub scrollback: ScrollbackConfig,
    pub cursor: CursorConfig,
    pub bell: BellConfig,
    pub colors: ColorsConfig,
    pub prompt: PromptConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub line_height: f32,
    pub ui_family: String,
    pub ui_size: f32,
    pub ligatures: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowConfig {
    pub padding_x: u16,
    pub padding_y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScrollbackConfig {
    pub lines: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CursorConfig {
    pub style: CursorStyle,
    pub blink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BellConfig {
    pub audible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorsConfig {
    #[serde(deserialize_with = "hex_rgb")]
    pub background: Option<Rgb>,
    #[serde(deserialize_with = "hex_rgb")]
    pub foreground: Option<Rgb>,
    #[serde(deserialize_with = "hex_rgb")]
    pub cursor: Option<Rgb>,
    #[serde(deserialize_with = "hex_rgba")]
    pub selection: Option<Rgba>,
    #[serde(deserialize_with = "hex_rgb")]
    pub accent: Option<Rgb>,
    #[serde(deserialize_with = "hex_palette")]
    pub normal: Option<[Rgb; 8]>,
    #[serde(deserialize_with = "hex_palette")]
    pub bright: Option<[Rgb; 8]>,
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: PathBuf,
        error: io::Error,
    },
    Parse(toml::de::Error),
    Keys(String),
    NotPositive {
        key: &'static str,
        value: f32,
    },
    ThemeName(String),
    UnknownTheme(String),
    Theme {
        name: String,
        error: toml::de::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Read { path, error } => {
                let path = path.display();
                write!(f, "cannot read {path}: {error}")
            }
            ConfigError::Parse(error) => write!(f, "invalid config: {error}"),
            ConfigError::Keys(error) => write!(f, "invalid config: {error}"),
            ConfigError::NotPositive { key, value } => {
                write!(
                    f,
                    "invalid config: {key} must be a finite number greater than 0, got {value}"
                )
            }
            ConfigError::ThemeName(name) => write!(
                f,
                "invalid theme name {name:?}, expected up to {MAX_THEME_NAME} letters, digits, dots, underscores or dashes, not starting with a dot"
            ),
            ConfigError::UnknownTheme(name) => write!(f, "unknown theme {name:?}"),
            ConfigError::Theme { name, error } => write!(f, "invalid theme {name:?}: {error}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Read { error, .. } => Some(error),
            ConfigError::Parse(error) | ConfigError::Theme { error, .. } => Some(error),
            ConfigError::Keys(_)
            | ConfigError::NotPositive { .. }
            | ConfigError::ThemeName(_)
            | ConfigError::UnknownTheme(_) => None,
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join(".config")
            })
            .join("flair")
            .join("flair.toml")
    }

    pub fn from_toml(text: &str) -> Result<Config, ConfigError> {
        let config: Config = toml::from_str(text).map_err(ConfigError::Parse)?;
        config.keys.validate().map_err(ConfigError::Keys)?;
        positive("font.size", config.font.size)?;
        positive("font.line_height", config.font.line_height)?;
        positive("font.ui_size", config.font.ui_size)?;
        Ok(config)
    }

    pub fn load(path: &Path) -> (Config, Option<ConfigError>) {
        match fs::read_to_string(path) {
            Ok(text) => match Config::from_toml(&text) {
                Ok(config) => (config, None),
                Err(error) => (Config::default(), Some(error)),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Config::default(), None),
            Err(error) => (
                Config::default(),
                Some(ConfigError::Read {
                    path: path.to_owned(),
                    error,
                }),
            ),
        }
    }

    pub fn themes_dir(path: &Path) -> PathBuf {
        path.parent().unwrap_or(Path::new("")).join("themes")
    }

    pub fn theme(&self) -> Theme {
        let base = builtin_colors(&self.theme).unwrap_or_default();
        self.colors.over(&base).to_theme()
    }

    pub fn load_theme(&self, dir: &Path) -> (Theme, Option<ConfigError>) {
        let (base, error) = match self.theme_file(dir) {
            Ok(colors) => (colors, None),
            Err(error) => (ColorsConfig::default(), Some(error)),
        };
        (self.colors.over(&base).to_theme(), error)
    }

    pub fn with_theme(&self, name: &str) -> Config {
        Config {
            theme: name.to_owned(),
            ..self.clone()
        }
    }

    fn theme_file(&self, dir: &Path) -> Result<ColorsConfig, ConfigError> {
        let name = &self.theme;
        if !valid_theme_name(name) {
            return Err(ConfigError::ThemeName(name.clone()));
        }
        let path = dir.join(format!("{name}.toml"));
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|error| ConfigError::Theme {
                name: name.clone(),
                error,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                builtin_colors(name).ok_or_else(|| ConfigError::UnknownTheme(name.clone()))
            }
            Err(error) => Err(ConfigError::Read { path, error }),
        }
    }
}

impl ColorsConfig {
    pub fn from_toml(text: &str) -> Result<ColorsConfig, ConfigError> {
        toml::from_str(text).map_err(ConfigError::Parse)
    }

    pub fn over(&self, base: &ColorsConfig) -> ColorsConfig {
        ColorsConfig {
            background: self.background.or(base.background),
            foreground: self.foreground.or(base.foreground),
            cursor: self.cursor.or(base.cursor),
            selection: self.selection.or(base.selection),
            accent: self.accent.or(base.accent),
            normal: self.normal.or(base.normal),
            bright: self.bright.or(base.bright),
        }
    }

    pub fn to_theme(&self) -> Theme {
        let foreground = self.foreground.unwrap_or(FLAIR_DARK.foreground);
        let accent = self.accent.unwrap_or(FLAIR_DARK.accent);
        Theme {
            background: self.background.unwrap_or(FLAIR_DARK.background),
            foreground,
            cursor: self.cursor.unwrap_or(foreground),
            selection: self.selection.unwrap_or(Rgba {
                r: accent.r,
                g: accent.g,
                b: accent.b,
                a: SELECTION_ALPHA,
            }),
            accent,
            normal: self.normal.unwrap_or(FLAIR_DARK.normal),
            bright: self.bright.unwrap_or(FLAIR_DARK.bright),
        }
    }
}

pub fn theme_names(dir: &Path) -> Vec<String> {
    let files = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| theme_file_name(&entry.path()));
    BUILTIN_THEMES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .chain(files)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn theme_file_name(path: &Path) -> Option<String> {
    if path.extension()? != "toml" {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    valid_theme_name(stem).then(|| stem.to_owned())
}

fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_THEME_NAME
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn builtin_colors(name: &str) -> Option<ColorsConfig> {
    BUILTIN_THEMES
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .and_then(|(_, text)| ColorsConfig::from_toml(text).ok())
}

impl Default for Config {
    fn default() -> Config {
        Config {
            theme: "flair-dark".to_owned(),
            keys: KeysConfig::default(),
            shell: ShellConfig::default(),
            env: default_env(),
            font: FontConfig::default(),
            window: WindowConfig::default(),
            scrollback: ScrollbackConfig::default(),
            cursor: CursorConfig::default(),
            bell: BellConfig::default(),
            colors: ColorsConfig::default(),
            prompt: PromptConfig::default(),
        }
    }
}

impl Default for ShellConfig {
    fn default() -> ShellConfig {
        ShellConfig {
            program: "/bin/zsh".to_owned(),
            args: vec!["-l".to_owned()],
        }
    }
}

impl Default for FontConfig {
    fn default() -> FontConfig {
        FontConfig {
            family: "JetBrainsMono Nerd Font Mono".to_owned(),
            size: 13.0,
            line_height: 1.0,
            ui_family: "sans-serif".to_owned(),
            ui_size: 12.0,
            ligatures: false,
        }
    }
}

impl Default for WindowConfig {
    fn default() -> WindowConfig {
        WindowConfig {
            padding_x: 8,
            padding_y: 0,
        }
    }
}

impl Default for ScrollbackConfig {
    fn default() -> ScrollbackConfig {
        ScrollbackConfig { lines: 10_000 }
    }
}

impl Default for CursorConfig {
    fn default() -> CursorConfig {
        CursorConfig {
            style: CursorStyle::Block,
            blink: false,
        }
    }
}

const FLAIR_DARK: Theme = Theme {
    background: rgb(0x03, 0x03, 0x03),
    foreground: rgb(0xd4, 0xd4, 0xd4),
    cursor: rgb(0xd4, 0xd4, 0xd4),
    selection: Rgba {
        r: 0x19,
        g: 0xaa,
        b: 0xd8,
        a: SELECTION_ALPHA,
    },
    accent: rgb(0x19, 0xaa, 0xd8),
    normal: [
        rgb(0x1e, 0x1e, 0x1e),
        rgb(0xe0, 0x6c, 0x75),
        rgb(0x98, 0xc3, 0x79),
        rgb(0xe5, 0xc0, 0x7b),
        rgb(0x61, 0xaf, 0xef),
        rgb(0xc6, 0x78, 0xdd),
        rgb(0x56, 0xb6, 0xc2),
        rgb(0xd4, 0xd4, 0xd4),
    ],
    bright: [
        rgb(0x5c, 0x63, 0x70),
        rgb(0xff, 0x7a, 0x85),
        rgb(0xb5, 0xe8, 0x90),
        rgb(0xff, 0xd6, 0x8a),
        rgb(0x7c, 0xc4, 0xff),
        rgb(0xde, 0x8c, 0xff),
        rgb(0x6f, 0xd6, 0xe2),
        rgb(0xff, 0xff, 0xff),
    ],
};

fn positive(key: &'static str, value: f32) -> Result<(), ConfigError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ConfigError::NotPositive { key, value })
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

fn default_env() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("TERM".to_owned(), "xterm-256color".to_owned()),
        ("COLORTERM".to_owned(), "truecolor".to_owned()),
    ])
}

fn env_over_defaults<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    let overrides = BTreeMap::<String, String>::deserialize(deserializer)?;
    Ok(default_env().into_iter().chain(overrides).collect())
}

fn hex_rgb<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Rgb>, D::Error> {
    let value = String::deserialize(deserializer)?;
    Rgb::from_hex(&value).map(Some).map_err(de::Error::custom)
}

fn hex_rgba<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Rgba>, D::Error> {
    let value = String::deserialize(deserializer)?;
    Rgba::from_hex(&value).map(Some).map_err(de::Error::custom)
}

fn hex_palette<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<[Rgb; 8]>, D::Error> {
    let values = <[String; 8]>::deserialize(deserializer)?;
    let mut palette = [Rgb::default(); 8];
    for (slot, value) in palette.iter_mut().zip(&values) {
        *slot = Rgb::from_hex(value).map_err(de::Error::custom)?;
    }
    Ok(Some(palette))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
