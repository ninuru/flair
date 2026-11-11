mod chrome;
mod context;
mod pipeline;

use crate::{
    atlas::{AtlasFull, AtlasGlyph, Face, GlyphCache, GlyphKey},
    cellgrid::CellGrid,
    font::{CellMetrics, Fonts, PROMPT_ICON_SCALE, Style},
    shape::{self, Shaped},
    shaper::Shaper,
    ui::px,
};
pub use chrome::{Border, Chrome, PxRect, Quad, TextFace, TextRun};
pub use context::{HeadlessContext, WindowContext};
use flair_core::{
    geom::CellPoint,
    proto::{CellFlags, CursorShape},
    theme::{Rgb, Rgba},
};
use glow::HasContext;
use pipeline::Pipeline;
use std::{collections::HashSet, error::Error, fmt, sync::Arc};
use unicode_width::UnicodeWidthChar;

const ATLAS_SIZE: u32 = 2048;

pub struct Renderer {
    gl: glow::Context,
    pipeline: Pipeline,
    metrics: CellMetrics,
    px: (u32, u32),
    cache: GlyphCache,
    uploaded: HashSet<GlyphKey>,
    reset_atlas: bool,
    shaper: Option<Shaper>,
}

#[derive(Debug)]
pub struct RenderError(pub String);
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for RenderError {}

fn instance(out: &mut Vec<f32>, rect: [f32; 4], uv: [f32; 4], color: Rgba, kind: f32) {
    let alpha = f32::from(color.a) / 255.0;
    out.extend(rect);
    out.extend(uv);
    out.extend([
        f32::from(color.r) / 255.0 * alpha,
        f32::from(color.g) / 255.0 * alpha,
        f32::from(color.b) / 255.0 * alpha,
        alpha,
        kind,
    ]);
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

fn rounded(out: &mut Vec<f32>, rect: PxRect, radius: f32, color: Rgba) {
    let bounds = [rect.x, rect.y, rect.w, rect.h];
    if radius > 0.0 {
        instance(out, bounds, [radius, 0.0, 0.0, 0.0], color, 3.0);
    } else {
        instance(out, bounds, [0.0; 4], color, 0.0);
    }
}

fn quad(out: &mut Vec<f32>, quad: &Quad) {
    let Some(border) = quad.border else {
        rounded(out, quad.rect, quad.radius, quad.color);
        return;
    };
    let width = border.width;
    let rect = quad.rect;
    rounded(out, rect, quad.radius, border.color);
    rounded(
        out,
        PxRect {
            x: rect.x + width,
            y: rect.y + width,
            w: (rect.w - width * 2.0).max(0.0),
            h: (rect.h - width * 2.0).max(0.0),
        },
        (quad.radius - width).max(0.0),
        quad.color,
    );
}

fn glyph_instance(out: &mut Vec<f32>, glyph: &AtlasGlyph, pen: (f32, f32), color: Rgb) {
    let r = glyph.rect;
    let a = ATLAS_SIZE as f32;
    instance(
        out,
        [
            pen.0 + glyph.raster.left as f32,
            pen.1 - glyph.raster.top as f32,
            r.w as f32,
            r.h as f32,
        ],
        [
            r.x as f32 / a,
            r.y as f32 / a,
            r.w as f32 / a,
            r.h as f32 / a,
        ],
        opaque(color),
        if glyph.raster.colored { 2.0 } else { 1.0 },
    );
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
            shaper: None,
        })
    }
    pub fn resize(&mut self, px: (u32, u32)) {
        self.px = (px.0.max(1), px.1.max(1));
    }
    pub fn set_metrics(&mut self, metrics: CellMetrics) {
        self.metrics = metrics;
        self.reset_atlas = true;
    }
    pub fn draw_frame(
        &mut self,
        grid: &CellGrid,
        chrome: &Chrome,
        fonts: &mut Fonts,
        background: Rgb,
    ) -> Result<(), RenderError> {
        self.sync_shaper(fonts);
        if self.reset_atlas {
            self.cache.clear();
            self.uploaded.clear();
            self.reset_atlas = false;
        }
        let margins = self.margins(grid, chrome.origin, chrome.extend);
        let cells = self.cells(grid, chrome.origin, chrome.scale, fonts);
        let mut quads = Vec::new();
        for q in &chrome.quads {
            quad(&mut quads, q);
        }
        let text = self.text(&chrome.text, fonts);
        let mut modal = Vec::new();
        for q in &chrome.modal {
            quad(&mut modal, q);
        }
        let modal_text = self.text(&chrome.modal_text, fonts);
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
        }
        self.scissor(chrome.extend);
        self.pipeline.draw(&self.gl, self.px, &margins);
        self.scissor(chrome.clip);
        for instances in &cells {
            self.pipeline.draw(&self.gl, self.px, instances);
        }
        unsafe {
            self.gl.disable(glow::SCISSOR_TEST);
        }
        for instances in [&quads, &text, &modal, &modal_text] {
            self.pipeline.draw(&self.gl, self.px, instances);
        }
        let error = unsafe { self.gl.get_error() };
        if error != glow::NO_ERROR {
            return Err(RenderError(format!("OpenGL draw error {error:#x}")));
        }
        Ok(())
    }
    fn sync_shaper(&mut self, fonts: &Fonts) {
        match (fonts.shaping(), &self.shaper) {
            (None, None) => {}
            (None, Some(_)) => self.shaper = None,
            (Some(source), Some(shaper)) if shaper.serves(source) => {}
            (Some(source), _) => {
                self.shaper = Some(Shaper::new(Arc::clone(source)));
                self.reset_atlas = true;
            }
        }
    }
    fn scissor(&self, clip: PxRect) {
        let (x, y, w, h) = scissor_box(clip, self.px.1);
        unsafe {
            self.gl.enable(glow::SCISSOR_TEST);
            self.gl.scissor(x, y, w, h);
        }
    }
    fn glyph(&mut self, key: GlyphKey, fonts: &mut Fonts) -> Option<AtlasGlyph> {
        let glyph = match self.cache.get_or_insert(key, || match key.face {
            Face::Cell => fonts.rasterize(key.c, key.style),
            Face::Ui => fonts.rasterize_ui(key.c, key.style),
            Face::Small => fonts.rasterize_small(key.c, key.style),
            Face::Icon => fonts.rasterize_icon(key.c),
            Face::PromptIcon => fonts.rasterize_prompt_icon(key.c),
            Face::Shaped(id) => self.shaper.as_ref()?.rasterize(id, key.style),
        }) {
            Ok(glyph) => glyph?,
            Err(AtlasFull) => {
                self.reset_atlas = true;
                return None;
            }
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
        Some(glyph)
    }
    fn text(&mut self, runs: &[TextRun], fonts: &mut Fonts) -> Vec<f32> {
        let mut out = Vec::new();
        for run in runs {
            let style = if run.bold {
                Style::Bold
            } else {
                Style::Regular
            };
            let face = match run.face {
                TextFace::Ui => Face::Ui,
                TextFace::Small => Face::Small,
                TextFace::Mono => Face::Cell,
                TextFace::Icon => Face::Icon,
                TextFace::PromptIcon => Face::PromptIcon,
            };
            let mut x = run.x;
            let mut base_x = x;
            for c in run.text.chars() {
                let width = c.width().unwrap_or(1);
                let pen_x = match run.face {
                    TextFace::Mono if width == 0 => base_x,
                    TextFace::Mono
                    | TextFace::Ui
                    | TextFace::Small
                    | TextFace::Icon
                    | TextFace::PromptIcon => x,
                };
                let key = GlyphKey { face, c, style };
                if let Some(glyph) = (c != ' ').then(|| self.glyph(key, fonts)).flatten() {
                    glyph_instance(
                        &mut out,
                        &glyph,
                        (pen_x.round(), run.baseline.round()),
                        run.color,
                    );
                }
                if width > 0 {
                    base_x = x;
                }
                x += match run.face {
                    TextFace::Ui => fonts.ui_advance(c, style),
                    TextFace::Small => fonts.small_advance(c, style),
                    TextFace::Mono => self.metrics.width * width as f32,
                    TextFace::Icon => fonts.ui_metrics().icon.width,
                    TextFace::PromptIcon => self.metrics.width * PROMPT_ICON_SCALE * width as f32,
                };
            }
        }
        out
    }
    fn margins(&self, grid: &CellGrid, origin: (f32, f32), extend: PxRect) -> Vec<f32> {
        let mut out = Vec::new();
        let m = self.metrics;
        let cols = usize::from(grid.size.cols);
        let rows = usize::from(grid.size.rows);
        if cols == 0 || rows == 0 || grid.cells.len() < cols * rows {
            return out;
        }
        let right = origin.0 + cols as f32 * m.width;
        let bottom = origin.1 + rows as f32 * m.height;
        let xs = [
            (extend.x, origin.0 - extend.x),
            (right, extend.x + extend.w - right),
        ];
        let ys = [
            (extend.y, origin.1 - extend.y),
            (bottom, extend.y + extend.h - bottom),
        ];
        let bg = |col: usize, row: usize| opaque(grid.cells[row * cols + col].bg);
        let edge = |row: usize| {
            dominant(
                grid.cells[row * cols..(row + 1) * cols]
                    .iter()
                    .map(|cell| cell.bg),
            )
        };
        let (Some(top), Some(bottom)) = (edge(0), edge(rows - 1)) else {
            return out;
        };
        let mut push = |x: (f32, f32), y: (f32, f32), color| {
            if x.1 > 0.0 && y.1 > 0.0 {
                instance(&mut out, [x.0, y.0, x.1, y.1], [0.0; 4], color, 0.0);
            }
        };
        for row in 0..rows {
            let y = (origin.1 + row as f32 * m.height, m.height);
            push(xs[0], y, bg(0, row));
            push(xs[1], y, bg(cols - 1, row));
        }
        let width = (extend.x, extend.w);
        push(width, ys[0], opaque(top));
        push(width, ys[1], opaque(bottom));
        out
    }
    fn cells(
        &mut self,
        grid: &CellGrid,
        origin: (f32, f32),
        scale: f32,
        fonts: &mut Fonts,
    ) -> [Vec<f32>; 3] {
        let thin = px(1.0, scale);
        let thick = px(2.0, scale);
        let mut rects = Vec::new();
        let mut glyphs = Vec::new();
        let mut decorations = Vec::new();
        let m = self.metrics;
        let cols = usize::from(grid.size.cols).max(1);
        let block = grid
            .cursor
            .filter(|cursor| cursor.shape == CursorShape::Block);
        let mut shaped = Vec::new();
        for (i, cell) in grid.cells.iter().enumerate() {
            if i % cols == 0
                && let Some(shaper) = &mut self.shaper
            {
                let row = &grid.cells[i..(i + cols).min(grid.cells.len())];
                let cursor = block
                    .filter(|cursor| usize::from(cursor.point.row) == i / cols)
                    .map(|cursor| usize::from(cursor.point.col));
                shaped = shaper.row(row, cursor);
            }
            let point = CellPoint {
                col: (i % cols) as u16,
                row: (i / cols) as u16,
            };
            let x = origin.0 + (i % cols) as f32 * m.width;
            let y = origin.1 + (i / cols) as f32 * m.height;
            instance(
                &mut rects,
                [x, y, m.width, m.height],
                [0.0; 4],
                opaque(cell.bg),
                0.0,
            );
            if cell.flags.contains(CellFlags::HIDDEN) || cell.flags.contains(CellFlags::WIDE_SPACER)
            {
                continue;
            }
            let style = shape::style(cell.flags);
            let cursor_here = grid
                .cursor
                .is_some_and(|cursor| cursor.point == point && cursor.shape == CursorShape::Block);
            let fg = if cursor_here { cell.bg } else { cell.fg };
            let width = m.width
                * if cell.flags.contains(CellFlags::WIDE) {
                    2.0
                } else {
                    1.0
                };
            let baseline = (y + m.baseline).round();
            match shaped.get(i % cols) {
                Some(Some(Shaped::Glyph(id))) => {
                    let key = GlyphKey {
                        face: Face::Shaped(*id),
                        c: cell.c,
                        style,
                    };
                    if let Some(glyph) = self.glyph(key, fonts) {
                        glyph_instance(&mut glyphs, &glyph, (x, baseline), fg);
                    }
                }
                Some(Some(Shaped::Blank)) => {}
                Some(None) | None => {
                    for (index, c) in std::iter::once(cell.c)
                        .chain(cell.extra.iter().flat_map(|s| s.chars()))
                        .enumerate()
                    {
                        if c == ' ' {
                            continue;
                        }
                        let key = GlyphKey {
                            face: Face::Cell,
                            c,
                            style,
                        };
                        let Some(glyph) = self.glyph(key, fonts) else {
                            continue;
                        };
                        let left = x + if index == 0 { 0.0 } else { width };
                        glyph_instance(&mut glyphs, &glyph, (left, baseline), fg);
                    }
                }
            }
            if cell.flags.contains(CellFlags::UNDERLINE) {
                instance(
                    &mut decorations,
                    [x, y + m.height - thick, width, thin],
                    [0.0; 4],
                    opaque(fg),
                    0.0,
                );
            }
            if cell.flags.contains(CellFlags::STRIKE) {
                instance(
                    &mut decorations,
                    [x, y + m.height / 2.0, width, thin],
                    [0.0; 4],
                    opaque(fg),
                    0.0,
                );
            }
        }
        if let Some(cursor) = grid.cursor {
            let x = origin.0 + f32::from(cursor.point.col) * m.width;
            let y = origin.1 + f32::from(cursor.point.row) * m.height;
            let color = opaque(cursor.color);
            match cursor.shape {
                CursorShape::Block => {
                    instance(&mut rects, [x, y, m.width, m.height], [0.0; 4], color, 0.0);
                }
                CursorShape::Beam => {
                    instance(&mut rects, [x, y, thick, m.height], [0.0; 4], color, 0.0);
                }
                CursorShape::Underline => instance(
                    &mut rects,
                    [x, y + m.height - thick, m.width, thick],
                    [0.0; 4],
                    color,
                    0.0,
                ),
                CursorShape::HollowBlock => {
                    for rect in [
                        [x, y, m.width, thin],
                        [x, y + m.height - thin, m.width, thin],
                        [x, y, thin, m.height],
                        [x + m.width - thin, y, thin, m.height],
                    ] {
                        instance(&mut rects, rect, [0.0; 4], color, 0.0);
                    }
                }
            }
        }
        [rects, glyphs, decorations]
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

fn dominant(colors: impl Iterator<Item = Rgb>) -> Option<Rgb> {
    let mut counts: Vec<(Rgb, usize)> = Vec::new();
    for color in colors {
        match counts.iter_mut().find(|(seen, _)| *seen == color) {
            Some((_, count)) => *count += 1,
            None => counts.push((color, 1)),
        }
    }
    counts
        .into_iter()
        .fold(
            None,
            |best: Option<(Rgb, usize)>, (color, count)| match best {
                Some((_, most)) if most >= count => best,
                _ => Some((color, count)),
            },
        )
        .map(|(color, _)| color)
}

fn scissor_box(clip: PxRect, height: u32) -> (i32, i32, i32, i32) {
    let left = clip.x.floor();
    let top = clip.y.floor();
    let right = (clip.x + clip.w).ceil();
    let bottom = (clip.y + clip.h).ceil();
    (
        left as i32,
        (height as f32 - bottom) as i32,
        (right - left) as i32,
        (bottom - top) as i32,
    )
}

#[cfg(test)]
#[path = "smoke_tests.rs"]
mod tests;
