use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, de};

use crate::{
    keys::KeysConfig,
    theme::{Rgb, Rgba, Theme},
};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorsConfig {
    #[serde(deserialize_with = "hex_rgb")]
    pub background: Rgb,
    #[serde(deserialize_with = "hex_rgb")]
    pub foreground: Rgb,
    #[serde(deserialize_with = "hex_rgb")]
    pub cursor: Rgb,
    #[serde(deserialize_with = "hex_rgba")]
    pub selection: Rgba,
    #[serde(deserialize_with = "hex_rgb")]
    pub accent: Rgb,
    #[serde(deserialize_with = "hex_palette")]
    pub normal: [Rgb; 8],
    #[serde(deserialize_with = "hex_palette")]
    pub bright: [Rgb; 8],
}

#[derive(Debug)]
pub enum ConfigError {
    Read { path: PathBuf, error: io::Error },
    Parse(toml::de::Error),
    Keys(String),
    NotPositive { key: &'static str, value: f32 },
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
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Read { error, .. } => Some(error),
            ConfigError::Parse(error) => Some(error),
            ConfigError::Keys(_) | ConfigError::NotPositive { .. } => None,
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

    pub fn theme(&self) -> Theme {
        let colors = &self.colors;
        Theme {
            background: colors.background,
            foreground: colors.foreground,
            cursor: colors.cursor,
            selection: colors.selection,
            accent: colors.accent,
            normal: colors.normal,
            bright: colors.bright,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            keys: KeysConfig::default(),
            shell: ShellConfig::default(),
            env: default_env(),
            font: FontConfig::default(),
            window: WindowConfig::default(),
            scrollback: ScrollbackConfig::default(),
            cursor: CursorConfig::default(),
            bell: BellConfig::default(),
            colors: ColorsConfig::default(),
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
        }
    }
}

impl Default for WindowConfig {
    fn default() -> WindowConfig {
        WindowConfig {
            padding_x: 16,
            padding_y: 8,
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

impl Default for ColorsConfig {
    fn default() -> ColorsConfig {
        ColorsConfig {
            background: rgb(0x03, 0x03, 0x03),
            foreground: rgb(0xd4, 0xd4, 0xd4),
            cursor: rgb(0xd4, 0xd4, 0xd4),
            selection: Rgba {
                r: 0x19,
                g: 0xaa,
                b: 0xd8,
                a: 0x40,
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
        }
    }
}

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

fn hex_rgb<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
    let value = String::deserialize(deserializer)?;
    Rgb::from_hex(&value).map_err(de::Error::custom)
}

fn hex_rgba<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgba, D::Error> {
    let value = String::deserialize(deserializer)?;
    Rgba::from_hex(&value).map_err(de::Error::custom)
}

fn hex_palette<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[Rgb; 8], D::Error> {
    let values = <[String; 8]>::deserialize(deserializer)?;
    let mut palette = [Rgb::default(); 8];
    for (slot, value) in palette.iter_mut().zip(&values) {
        *slot = Rgb::from_hex(value).map_err(de::Error::custom)?;
    }
    Ok(palette)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
