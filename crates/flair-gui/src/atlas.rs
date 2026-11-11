use crate::font::{RasterGlyph, Style};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub struct Atlas {
    size: u32,
    x: u32,
    y: u32,
    row_height: u32,
}

impl Atlas {
    pub fn new(size: u32) -> Self {
        Self {
            size,
            x: 0,
            y: 0,
            row_height: 0,
        }
    }
    pub fn insert(&mut self, w: u32, h: u32) -> Option<AtlasRect> {
        if w == 0 || h == 0 || w > self.size || h > self.size {
            return None;
        }
        let (x, y, row_height) = if self.x + w > self.size {
            (0, self.y + self.row_height, 0)
        } else {
            (self.x, self.y, self.row_height)
        };
        if y + h > self.size {
            return None;
        }
        self.x = x + w;
        self.y = y;
        self.row_height = row_height.max(h);
        Some(AtlasRect { x, y, w, h })
    }
    pub fn clear(&mut self) {
        self.x = 0;
        self.y = 0;
        self.row_height = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Cell,
    Ui,
    Small,
    Icon,
    PromptIcon,
    Shaped(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub face: Face,
    pub c: char,
    pub style: Style,
}

#[derive(Clone)]
pub struct AtlasGlyph {
    pub rect: AtlasRect,
    pub raster: Arc<RasterGlyph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasFull;

pub struct GlyphCache {
    atlas: Atlas,
    glyphs: HashMap<GlyphKey, AtlasGlyph>,
}

impl GlyphCache {
    pub fn new(size: u32) -> Self {
        Self {
            atlas: Atlas::new(size),
            glyphs: HashMap::new(),
        }
    }
    pub fn get_or_insert(
        &mut self,
        key: GlyphKey,
        raster: impl FnOnce() -> Option<RasterGlyph>,
    ) -> Result<Option<AtlasGlyph>, AtlasFull> {
        if let Some(glyph) = self.glyphs.get(&key) {
            return Ok(Some(glyph.clone()));
        }
        let Some(raster) = raster().filter(|raster| raster.width > 0 && raster.height > 0) else {
            return Ok(None);
        };
        let rect = self
            .atlas
            .insert(raster.width, raster.height)
            .ok_or(AtlasFull)?;
        let glyph = AtlasGlyph {
            rect,
            raster: Arc::new(raster),
        };
        self.glyphs.insert(key, glyph.clone());
        Ok(Some(glyph))
    }
    pub fn clear(&mut self) {
        self.atlas.clear();
        self.glyphs.clear();
    }
}

#[cfg(test)]
#[path = "atlas_tests.rs"]
mod tests;
