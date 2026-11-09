use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::proto::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorError(pub String);

impl fmt::Display for ColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = &self.0;
        write!(
            f,
            "invalid colour {value:?}, expected a hex colour like #rrggbb"
        )
    }
}

impl Error for ColorError {}

impl Rgb {
    pub fn from_hex(value: &str) -> Result<Rgb, ColorError> {
        hex_bytes(value)
            .map(|[r, g, b]| Rgb { r, g, b })
            .ok_or_else(|| ColorError(value.to_owned()))
    }
}

impl Rgba {
    pub fn from_hex(value: &str) -> Result<Rgba, ColorError> {
        hex_bytes(value)
            .map(|[r, g, b, a]| Rgba { r, g, b, a })
            .or_else(|| hex_bytes(value).map(|[r, g, b]| Rgba { r, g, b, a: 255 }))
            .ok_or_else(|| ColorError(value.to_owned()))
    }
}

fn hex_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != N * 2 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub background: Rgb,
    pub foreground: Rgb,
    pub cursor: Rgb,
    pub selection: Rgba,
    pub accent: Rgb,
    pub normal: [Rgb; 8],
    pub bright: [Rgb; 8],
}

impl Theme {
    pub fn resolve(&self, color: Color) -> Rgb {
        match color {
            Color::Foreground => self.foreground,
            Color::Background => self.background,
            Color::Cursor => self.cursor,
            Color::Indexed(i) => self.indexed(i),
            Color::Rgb(r, g, b) => Rgb { r, g, b },
        }
    }

    pub fn indexed(&self, i: u8) -> Rgb {
        match i {
            0..=7 => self.normal[usize::from(i)],
            8..=15 => self.bright[usize::from(i - 8)],
            16..=231 => {
                let n = i - 16;
                Rgb {
                    r: cube_level(n / 36),
                    g: cube_level(n / 6 % 6),
                    b: cube_level(n % 6),
                }
            }
            232..=255 => {
                let v = 8 + 10 * (i - 232);
                Rgb { r: v, g: v, b: v }
            }
        }
    }
}

pub const BUILTIN_THEMES: [(&str, &str); 3] = [
    (
        "catppuccin-mocha",
        include_str!("themes/catppuccin-mocha.toml"),
    ),
    ("flair-dark", include_str!("themes/flair-dark.toml")),
    ("gruvbox-dark", include_str!("themes/gruvbox-dark.toml")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surfaces {
    pub surface1: Rgb,
    pub surface2: Rgb,
    pub surface3: Rgb,
    pub outline: Rgb,
    pub pane_border: Rgb,
    pub muted: Rgb,
    pub accent_soft: Rgb,
}

impl Theme {
    pub fn surfaces(&self) -> Surfaces {
        let tint = |percent| blend(self.background, self.foreground, percent);
        Surfaces {
            surface1: tint(5),
            surface2: tint(10),
            surface3: tint(15),
            outline: tint(10),
            pane_border: tint(15),
            muted: tint(60),
            accent_soft: blend(self.background, self.accent, 25),
        }
    }
}

pub fn blend(under: Rgb, over: Rgb, percent: u8) -> Rgb {
    let p = u16::from(percent.min(100));
    let mix = |under: u8, over: u8| {
        u8::try_from((u16::from(under) * (100 - p) + u16::from(over) * p + 50) / 100)
            .unwrap_or(u8::MAX)
    };
    Rgb {
        r: mix(under.r, over.r),
        g: mix(under.g, over.g),
        b: mix(under.b, over.b),
    }
}

fn cube_level(level: u8) -> u8 {
    if level == 0 { 0 } else { 55 + 40 * level }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
