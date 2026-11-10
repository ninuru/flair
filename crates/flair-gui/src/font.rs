use std::{collections::HashMap, error::Error, fmt};

use crossfont::{BitmapBuffer, FontDesc, Rasterize, Slant, Weight};
use flair_core::config::FontConfig;

use crate::ui::SMALL_TEXT;

pub struct Fonts {
    rasterizer: crossfont::Rasterizer,
    faces: [crossfont::FontKey; 4],
    size: crossfont::Size,
    metrics: CellMetrics,
    ui_faces: [crossfont::FontKey; 2],
    ui_size: crossfont::Size,
    small_size: crossfont::Size,
    ui_metrics: UiMetrics,
    ui_advances: HashMap<(char, Style, bool), f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiMetrics {
    pub height: f32,
    pub ascent: f32,
    pub small: f32,
    pub cap: f32,
    pub icon: CellMetrics,
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
        let metrics = cell_metrics(&raw, config.line_height);
        let ui_size = crossfont::Size::new(config.ui_size * scale as f32);
        let mut ui_faces = Vec::new();
        for weight in [Weight::Normal, Weight::Bold] {
            let style = crossfont::Style::Description {
                slant: Slant::Normal,
                weight,
            };
            let face = rasterizer
                .load_font(&FontDesc::new(&config.ui_family, style.clone()), ui_size)
                .or_else(|_| {
                    log::warn!(
                        "font {} unavailable; using sans-serif fallback",
                        config.ui_family
                    );
                    rasterizer.load_font(&FontDesc::new("sans-serif", style), ui_size)
                })
                .map_err(|e| FontError(e.to_string()))?;
            ui_faces.push(face);
        }
        rasterizer
            .get_glyph(crossfont::GlyphKey {
                character: 'm',
                font_key: ui_faces[0],
                size: ui_size,
            })
            .map_err(|e| FontError(e.to_string()))?;
        let raw = rasterizer
            .metrics(ui_faces[0], ui_size)
            .map_err(|e| FontError(e.to_string()))?;
        let small_size = crossfont::Size::from_px(SMALL_TEXT * scale as f32);
        let cap = rasterizer
            .get_glyph(crossfont::GlyphKey {
                character: 'H',
                font_key: ui_faces[0],
                size: small_size,
            })
            .map_err(|e| FontError(e.to_string()))?
            .top as f32;
        rasterizer
            .get_glyph(crossfont::GlyphKey {
                character: 'm',
                font_key: faces[0],
                size: small_size,
            })
            .map_err(|e| FontError(e.to_string()))?;
        let icon = rasterizer
            .metrics(faces[0], small_size)
            .map_err(|e| FontError(e.to_string()))?;
        let ui_metrics = UiMetrics {
            height: (raw.line_height as f32).ceil().max(1.0),
            ascent: raw.line_height as f32 + raw.descent,
            small: small_size.as_px() / ui_size.as_px(),
            cap,
            icon: cell_metrics(&icon, 1.0),
        };
        Ok(Fonts {
            rasterizer,
            faces: faces.try_into().unwrap(),
            size,
            metrics,
            ui_faces: ui_faces.try_into().unwrap(),
            ui_size,
            small_size,
            ui_metrics,
            ui_advances: HashMap::new(),
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
        self.raster(key)
    }

    pub fn rasterize_icon(&mut self, c: char) -> Option<RasterGlyph> {
        self.raster(crossfont::GlyphKey {
            character: c,
            font_key: self.faces[0],
            size: self.small_size,
        })
    }

    pub fn ui_metrics(&self) -> UiMetrics {
        self.ui_metrics
    }

    pub fn ui_advance(&mut self, c: char, style: Style) -> f32 {
        self.advance(c, style, false)
    }

    pub fn small_advance(&mut self, c: char, style: Style) -> f32 {
        self.advance(c, style, true)
    }

    fn advance(&mut self, c: char, style: Style, small: bool) -> f32 {
        if let Some(advance) = self.ui_advances.get(&(c, style, small)) {
            return *advance;
        }
        let advance = self
            .glyph(self.ui_key(c, style, small))
            .map_or(0.0, |glyph| glyph.advance.0 as f32);
        self.ui_advances.insert((c, style, small), advance);
        advance
    }

    pub fn ui_width(&mut self, text: &str, bold: bool) -> f32 {
        let style = if bold { Style::Bold } else { Style::Regular };
        text.chars().map(|c| self.ui_advance(c, style)).sum()
    }

    pub fn rasterize_ui(&mut self, c: char, style: Style) -> Option<RasterGlyph> {
        self.raster(self.ui_key(c, style, false))
    }

    pub fn rasterize_small(&mut self, c: char, style: Style) -> Option<RasterGlyph> {
        self.raster(self.ui_key(c, style, true))
    }

    fn ui_key(&self, c: char, style: Style, small: bool) -> crossfont::GlyphKey {
        let index = match style {
            Style::Regular | Style::Italic => 0,
            Style::Bold | Style::BoldItalic => 1,
        };
        crossfont::GlyphKey {
            character: c,
            font_key: self.ui_faces[index],
            size: if small { self.small_size } else { self.ui_size },
        }
    }

    fn glyph(&mut self, key: crossfont::GlyphKey) -> Option<crossfont::RasterizedGlyph> {
        match self.rasterizer.get_glyph(key) {
            Ok(glyph) | Err(crossfont::Error::MissingGlyph(glyph)) => Some(glyph),
            Err(error) => {
                let c = key.character;
                log::debug!("glyph {c:?}: {error}");
                None
            }
        }
    }

    fn raster(&mut self, key: crossfont::GlyphKey) -> Option<RasterGlyph> {
        let glyph = self.glyph(key)?;
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

fn cell_metrics(raw: &crossfont::Metrics, line_height: f32) -> CellMetrics {
    let height = (raw.line_height as f32 * line_height).ceil().max(1.0);
    CellMetrics {
        width: (raw.average_advance as f32).ceil().max(1.0),
        height,
        baseline: height + raw.descent - (height - raw.line_height as f32) / 2.0,
    }
}
