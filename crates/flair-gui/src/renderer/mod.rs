mod context;
mod pipeline;

use crate::{
    atlas::{GlyphCache, GlyphKey},
    cellgrid::CellGrid,
    font::{CellMetrics, Fonts, Style},
};
pub use context::{HeadlessContext, WindowContext};
use flair_core::{
    proto::{CellFlags, CursorShape},
    theme::Rgb,
};
use glow::HasContext;
use pipeline::Pipeline;
use std::{collections::HashSet, error::Error, fmt};

const ATLAS_SIZE: u32 = 2048;

pub struct Renderer {
    gl: glow::Context,
    pipeline: Pipeline,
    metrics: CellMetrics,
    px: (u32, u32),
    cache: GlyphCache,
    uploaded: HashSet<GlyphKey>,
    reset_atlas: bool,
}

#[derive(Debug)]
pub struct RenderError(pub String);
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for RenderError {}

fn instance(out: &mut Vec<f32>, rect: [f32; 4], uv: [f32; 4], color: Rgb, kind: f32) {
    out.extend(rect);
    out.extend(uv);
    out.extend([
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        1.0,
        kind,
    ]);
}

impl Renderer {
    pub fn new(gl: glow::Context, metrics: CellMetrics) -> Result<Renderer, RenderError> {
        let pipeline = Pipeline::new(&gl, ATLAS_SIZE as i32)?;
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
        }
        Ok(Self {
            gl,
            pipeline,
            metrics,
            px: (1, 1),
            cache: GlyphCache::new(ATLAS_SIZE),
            uploaded: HashSet::new(),
            reset_atlas: false,
        })
    }
    pub fn resize(&mut self, px: (u32, u32)) {
        self.px = (px.0.max(1), px.1.max(1));
    }
    pub fn set_metrics(&mut self, metrics: CellMetrics) {
        self.metrics = metrics;
        self.reset_atlas = true;
    }
    pub fn draw(
        &mut self,
        grid: &CellGrid,
        fonts: &mut Fonts,
        padding: (f32, f32),
        background: Rgb,
    ) -> Result<(), RenderError> {
        if self.reset_atlas {
            self.cache.clear();
            self.uploaded.clear();
            self.reset_atlas = false;
        }
        let mut rects = Vec::new();
        let mut glyphs = Vec::new();
        let mut decorations = Vec::new();
        let m = self.metrics;
        for (i, cell) in grid.cells.iter().enumerate() {
            let cols = usize::from(grid.size.cols).max(1);
            let x = padding.0 + (i % cols) as f32 * m.width;
            let y = padding.1 + (i / cols) as f32 * m.height;
            instance(
                &mut rects,
                [x, y, m.width, m.height],
                [0.0; 4],
                cell.bg,
                0.0,
            );
            if cell.flags.contains(CellFlags::HIDDEN) || cell.flags.contains(CellFlags::WIDE_SPACER)
            {
                continue;
            }
            let style = match (
                cell.flags.contains(CellFlags::BOLD),
                cell.flags.contains(CellFlags::ITALIC),
            ) {
                (false, false) => Style::Regular,
                (true, false) => Style::Bold,
                (false, true) => Style::Italic,
                (true, true) => Style::BoldItalic,
            };
            let cursor_here = grid.cursor.is_some_and(|cursor| {
                usize::from(cursor.point.row) == i / cols
                    && usize::from(cursor.point.col) == i % cols
                    && cursor.shape == CursorShape::Block
            });
            let fg = if cursor_here { cell.bg } else { cell.fg };
            for (index, c) in std::iter::once(cell.c)
                .chain(cell.extra.iter().flat_map(|s| s.chars()))
                .enumerate()
            {
                if c == ' ' {
                    continue;
                }
                let key = GlyphKey { c, style };
                let Some(glyph) = self.cache.get_or_insert(key, || fonts.rasterize(c, style))
                else {
                    self.reset_atlas = true;
                    continue;
                };
                if self.uploaded.insert(key) {
                    unsafe {
                        self.gl
                            .bind_texture(glow::TEXTURE_2D, Some(self.pipeline.texture));
                        self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                        self.gl.tex_sub_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glyph.rect.x as i32,
                            glyph.rect.y as i32,
                            glyph.rect.w as i32,
                            glyph.rect.h as i32,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(&glyph.raster.pixels)),
                        );
                    }
                }
                let r = glyph.rect;
                let a = ATLAS_SIZE as f32;
                let origin = x + if index == 0 {
                    0.0
                } else {
                    m.width
                        * if cell.flags.contains(CellFlags::WIDE) {
                            2.0
                        } else {
                            1.0
                        }
                };
                instance(
                    &mut glyphs,
                    [
                        origin + glyph.raster.left as f32,
                        (y + m.baseline).round() - glyph.raster.top as f32,
                        r.w as f32,
                        r.h as f32,
                    ],
                    [
                        r.x as f32 / a,
                        r.y as f32 / a,
                        r.w as f32 / a,
                        r.h as f32 / a,
                    ],
                    fg,
                    if glyph.raster.colored { 2.0 } else { 1.0 },
                );
            }
            let width = m.width
                * if cell.flags.contains(CellFlags::WIDE) {
                    2.0
                } else {
                    1.0
                };
            if cell.flags.contains(CellFlags::UNDERLINE) {
                instance(
                    &mut decorations,
                    [x, y + m.height - 2.0, width, 1.0],
                    [0.0; 4],
                    fg,
                    0.0,
                );
            }
            if cell.flags.contains(CellFlags::STRIKE) {
                instance(
                    &mut decorations,
                    [x, y + m.height / 2.0, width, 1.0],
                    [0.0; 4],
                    fg,
                    0.0,
                );
            }
        }
        if let Some(cursor) = grid.cursor {
            let x = padding.0 + f32::from(cursor.point.col) * m.width;
            let y = padding.1 + f32::from(cursor.point.row) * m.height;
            match cursor.shape {
                CursorShape::Block => instance(
                    &mut rects,
                    [x, y, m.width, m.height],
                    [0.0; 4],
                    cursor.color,
                    0.0,
                ),
                CursorShape::Beam => instance(
                    &mut rects,
                    [x, y, 2.0, m.height],
                    [0.0; 4],
                    cursor.color,
                    0.0,
                ),
                CursorShape::Underline => instance(
                    &mut rects,
                    [x, y + m.height - 2.0, m.width, 2.0],
                    [0.0; 4],
                    cursor.color,
                    0.0,
                ),
                CursorShape::HollowBlock => {
                    for rect in [
                        [x, y, m.width, 1.0],
                        [x, y + m.height - 1.0, m.width, 1.0],
                        [x, y, 1.0, m.height],
                        [x + m.width - 1.0, y, 1.0, m.height],
                    ] {
                        instance(&mut rects, rect, [0.0; 4], cursor.color, 0.0);
                    }
                }
            }
        }
        unsafe {
            self.gl.viewport(0, 0, self.px.0 as i32, self.px.1 as i32);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.clear_color(
                f32::from(background.r) / 255.0,
                f32::from(background.g) / 255.0,
                f32::from(background.b) / 255.0,
                1.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(
                padding.0 as i32,
                padding.1 as i32,
                (self.px.0 as f32 - padding.0 * 2.0).max(0.0) as i32,
                (self.px.1 as f32 - padding.1 * 2.0).max(0.0) as i32,
            );
        }
        self.pipeline.draw(&self.gl, self.px, &rects);
        self.pipeline.draw(&self.gl, self.px, &glyphs);
        self.pipeline.draw(&self.gl, self.px, &decorations);
        unsafe {
            self.gl.disable(glow::SCISSOR_TEST);
        }
        let error = unsafe { self.gl.get_error() };
        if error != glow::NO_ERROR {
            return Err(RenderError(format!("OpenGL draw error {error:#x}")));
        }
        Ok(())
    }
    pub fn read_pixels(&self) -> Vec<u8> {
        let mut pixels = vec![0; self.px.0 as usize * self.px.1 as usize * 4];
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                self.px.0 as i32,
                self.px.1 as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
        }
        pixels
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        self.pipeline.destroy(&self.gl);
    }
}

#[cfg(test)]
#[path = "smoke_tests.rs"]
mod tests;
