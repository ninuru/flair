use std::{error::Error, fmt};

use flair_core::config::FontConfig;

pub struct Fonts {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterGlyph {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub colored: bool,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub struct FontError(pub String);

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for FontError {}

impl Fonts {
    pub fn load(config: &FontConfig, scale: f64) -> Result<Fonts, FontError> {
        todo!()
    }

    pub fn metrics(&self) -> CellMetrics {
        todo!()
    }

    pub fn rasterize(&mut self, c: char, style: Style) -> Option<RasterGlyph> {
        todo!()
    }
}
