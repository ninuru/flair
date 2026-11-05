use std::{error::Error, fmt};

use crossfont::{BitmapBuffer, FontDesc, Rasterize, Slant, Weight};
use flair_core::config::FontConfig;

pub struct Fonts {
    rasterizer: crossfont::Rasterizer,
    faces: [crossfont::FontKey; 4],
    size: crossfont::Size,
    metrics: CellMetrics,
}

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
        let mut rasterizer = crossfont::Rasterizer::new().map_err(|e| FontError(e.to_string()))?;
        let size = crossfont::Size::new(config.size * scale as f32);
        let styles = [
            (Slant::Normal, Weight::Normal),
            (Slant::Normal, Weight::Bold),
            (Slant::Italic, Weight::Normal),
            (Slant::Italic, Weight::Bold),
        ];
        let mut faces = Vec::new();
        for (slant, weight) in styles {
            let style = crossfont::Style::Description { slant, weight };
            let desc = FontDesc::new(&config.family, style.clone());
            let face = rasterizer
                .load_font(&desc, size)
                .or_else(|_| {
                    log::warn!(
                        "font {} unavailable; using monospace fallback",
                        config.family
                    );
                    rasterizer.load_font(&FontDesc::new("monospace", style), size)
                })
                .map_err(|e| FontError(e.to_string()))?;
            faces.push(face);
        }
        rasterizer
            .get_glyph(crossfont::GlyphKey {
                character: 'm',
                font_key: faces[0],
                size,
            })
            .map_err(|e| FontError(e.to_string()))?;
        let raw = rasterizer
            .metrics(faces[0], size)
            .map_err(|e| FontError(e.to_string()))?;
        let height = (raw.line_height as f32 * config.line_height)
            .ceil()
            .max(1.0);
        let metrics = CellMetrics {
            width: (raw.average_advance as f32).ceil().max(1.0),
            height,
            baseline: height + raw.descent - (height - raw.line_height as f32) / 2.0,
        };
        Ok(Fonts {
            rasterizer,
            faces: faces.try_into().unwrap(),
            size,
            metrics,
        })
    }

    pub fn metrics(&self) -> CellMetrics {
        self.metrics
    }

    pub fn rasterize(&mut self, c: char, style: Style) -> Option<RasterGlyph> {
        let index = match style {
            Style::Regular => 0,
            Style::Bold => 1,
            Style::Italic => 2,
            Style::BoldItalic => 3,
        };
        let key = crossfont::GlyphKey {
            character: c,
            font_key: self.faces[index],
            size: self.size,
        };
        let glyph = match self.rasterizer.get_glyph(key) {
            Ok(glyph) | Err(crossfont::Error::MissingGlyph(glyph)) => glyph,
            Err(error) => {
                log::debug!("glyph {c:?}: {error}");
                return None;
            }
        };
        if glyph.width <= 0 || glyph.height <= 0 {
            return None;
        }
        let (colored, pixels) = match glyph.buffer {
            BitmapBuffer::Rgb(rgb) => (
                false,
                rgb.as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|p| {
                        [
                            255,
                            255,
                            255,
                            ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3) as u8,
                        ]
                    })
                    .collect(),
            ),
            BitmapBuffer::Rgba(rgba) => (true, rgba),
        };
        Some(RasterGlyph {
            width: glyph.width as u32,
            height: glyph.height as u32,
            left: glyph.left,
            top: glyph.top,
            colored,
            pixels,
        })
    }
}
