use flair_core::{
    config::WindowConfig,
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::{PaneId, WindowId},
    isles::Chip,
    proto::{Separator, WindowLayout},
    theme::Theme,
};

use crate::{
    CellMetrics, UiMetrics,
    chrome_bar::{self, Scene, fill},
    isles::{self, Isles},
    notice::Notice,
    overlay_layout::{OverlayHits, OverlayInput},
    overlays::Overlays,
    renderer::{Chrome, PxRect},
    status::StatusParts,
    ui::{BAR, LINE, px},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarField<'a> {
    pub label: &'a str,
    pub text: &'a str,
    pub cursor: usize,
}

pub struct ChromeInput<'a> {
    pub px: (u32, u32),
    pub scale: f32,
    pub padding: (f32, f32),
    pub zen: bool,
    pub metrics: CellMetrics,
    pub ui: UiMetrics,
    pub theme: &'a Theme,
    pub status: &'a StatusParts,
    pub prompt: Option<BarField<'a>>,
    pub layout: Option<&'a WindowLayout>,
    pub hover: Option<(f32, f32)>,
    pub overlays: Option<&'a Overlays>,
    pub notice: Option<&'a Notice>,
    pub isles: Option<Isles<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Window(WindowId),
    Session,
    Prompt { pane: PaneId, row: u16, chip: Chip },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub rect: PxRect,
    pub target: HitTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChromeLayout {
    pub chrome: Chrome,
    pub hits: Vec<Hit>,
    pub overlay: Option<OverlayHits>,
}

pub fn bar_height(scale: f32) -> f32 {
    px(BAR, scale)
}

pub fn chrome_top(scale: f32, zen: bool) -> f32 {
    match zen {
        true => 0.0,
        false => chrome_bottom(scale),
    }
}

pub fn padding(window: &WindowConfig, scale: f64, zen: bool) -> (f32, f32) {
    match zen {
        true => (0.0, 0.0),
        false => (
            (f32::from(window.padding_x) * scale as f32).round(),
            (f32::from(window.padding_y) * scale as f32).round(),
        ),
    }
}

pub fn grid_origin(
    px: (u32, u32),
    padding: (f32, f32),
    scale: f32,
    zen: bool,
    metrics: CellMetrics,
) -> (f32, f32) {
    let size = grid_size(px, padding, scale, zen, metrics);
    let top = chrome_top(scale, zen) + padding.1;
    let width = px.0 as f32 - padding.0 * 2.0 - f32::from(size.cols) * metrics.width;
    let height = px.1 as f32 - top - padding.1 - f32::from(size.rows) * metrics.height;
    (
        padding.0 + (width.max(0.0) / 2.0).floor(),
        top + height.max(0.0).floor(),
    )
}

pub fn grid_size(
    px: (u32, u32),
    padding: (f32, f32),
    scale: f32,
    zen: bool,
    metrics: CellMetrics,
) -> GridSize {
    let width = px.0 as f32 - padding.0 * 2.0;
    let height = px.1 as f32 - chrome_top(scale, zen) - padding.1 * 2.0;
    GridSize {
        cols: cells(width, metrics.width),
        rows: cells(height, metrics.height),
    }
}

pub fn hit_test(hits: &[Hit], pos: (f64, f64)) -> Option<HitTarget> {
    hits.iter()
        .find(|hit| {
            let rect = hit.rect;
            pos.0 >= f64::from(rect.x)
                && pos.0 < f64::from(rect.x + rect.w)
                && pos.1 >= f64::from(rect.y)
                && pos.1 < f64::from(rect.y + rect.h)
        })
        .map(|hit| hit.target)
}

pub fn layout(input: &ChromeInput, mut measure: impl FnMut(&str, bool) -> f32) -> ChromeLayout {
    let scale = input.scale;
    let width = input.px.0 as f32;
    let height = input.px.1 as f32;
    let mut scene = Scene::default();
    let open = input.overlays.is_some_and(Overlays::is_open);
    let bar_input = ChromeInput {
        hover: input.hover.filter(|_| !open),
        ..*input
    };
    if !input.zen {
        chrome_bar::bar(&bar_input, &mut scene, &mut measure);
    }
    let chrome = chrome_top(scale, input.zen);
    let origin = grid_origin(input.px, input.padding, scale, input.zen, input.metrics);
    let top = chrome + input.padding.1;
    let clip = PxRect {
        x: input.padding.0,
        y: top,
        w: (width - input.padding.0 * 2.0).max(0.0),
        h: (height - top - input.padding.1).max(0.0),
    };
    let extend = PxRect {
        x: 0.0,
        y: chrome,
        w: width,
        h: (height - chrome).max(0.0),
    };
    if let Some(window) = input.layout {
        separators(input, window, extend, &mut scene);
        isles::band(input, window, &mut scene);
    }
    if input.zen && (input.prompt.is_some() || input.notice.is_some()) {
        chrome_bar::bar(&bar_input, &mut scene, &mut measure);
    }
    let overlay = input.overlays.and_then(|overlays| {
        overlays.layout(
            &OverlayInput {
                metrics: input.metrics,
                px: input.px,
                scale,
                ui: input.ui,
                theme: input.theme,
                hover: input.hover,
            },
            &mut measure,
        )
    });
    let (modal, modal_text, overlay) = match overlay {
        Some(scene) => (scene.quads, scene.text, Some(scene.hits)),
        None => (Vec::new(), Vec::new(), None),
    };
    ChromeLayout {
        chrome: Chrome {
            scale,
            origin,
            clip,
            extend,
            quads: scene.quads,
            text: scene.text,
            modal,
            modal_text,
        },
        hits: scene.hits,
        overlay,
    }
}

fn separators(input: &ChromeInput, window: &WindowLayout, bounds: PxRect, scene: &mut Scene) {
    let theme = input.theme;
    let active = window
        .panes
        .iter()
        .find(|placement| placement.pane == window.active)
        .map(|placement| placement.outer());
    for separator in &window.separators {
        if let Some(rect) = intersect(
            separator_rect(input, separator.axis, separator.rect, bounds),
            bounds,
        ) {
            scene.quads.push(fill(rect, theme.surfaces().outline));
        }
    }
    for separator in &window.separators {
        if let Some(rect) = active
            .and_then(|pane| adjacent(separator, pane))
            .and_then(|segment| {
                intersect(
                    separator_rect(input, separator.axis, segment, bounds),
                    bounds,
                )
            })
        {
            scene.quads.push(fill(rect, theme.accent));
        }
    }
}

fn adjacent(separator: &Separator, pane: CellRect) -> Option<CellRect> {
    let gap = separator.rect;
    let (along, across) = match separator.axis {
        Axis::Horizontal => (
            (
                gap.origin.row,
                gap.size.rows,
                pane.origin.row,
                pane.size.rows,
            ),
            (
                gap.origin.col,
                gap.size.cols,
                pane.origin.col,
                pane.size.cols,
            ),
        ),
        Axis::Vertical => (
            (
                gap.origin.col,
                gap.size.cols,
                pane.origin.col,
                pane.size.cols,
            ),
            (
                gap.origin.row,
                gap.size.rows,
                pane.origin.row,
                pane.size.rows,
            ),
        ),
    };
    let (gap_start, gap_len, pane_start, pane_len) = across;
    let touches = pane_start + pane_len == gap_start || gap_start + gap_len == pane_start;
    let (gap_start_along, gap_len_along, pane_start_along, pane_len_along) = along;
    let first = gap_start_along.max(pane_start_along);
    let last = (gap_start_along + gap_len_along).min(pane_start_along + pane_len_along);
    (touches && last > first).then(|| match separator.axis {
        Axis::Horizontal => CellRect {
            origin: CellPoint {
                col: gap.origin.col,
                row: first,
            },
            size: GridSize {
                cols: gap.size.cols,
                rows: last - first,
            },
        },
        Axis::Vertical => CellRect {
            origin: CellPoint {
                col: first,
                row: gap.origin.row,
            },
            size: GridSize {
                cols: last - first,
                rows: gap.size.rows,
            },
        },
    })
}

fn separator_rect(input: &ChromeInput, axis: Axis, rect: CellRect, bounds: PxRect) -> PxRect {
    let gap = cell_rect(input, rect);
    let grid = grid_size(
        input.px,
        input.padding,
        input.scale,
        input.zen,
        input.metrics,
    );
    let metrics = input.metrics;
    let line = px(LINE, input.scale);
    let centre = |start: f32, length: f32| (start + (length - line) / 2.0).round();
    match axis {
        Axis::Horizontal => {
            let top = match rect.origin.row {
                0 => bounds.y,
                _ => centre(gap.y - metrics.height, metrics.height),
            };
            let bottom = match rect.origin.row + rect.size.rows >= grid.rows {
                true => bounds.y + bounds.h,
                false => centre(gap.y + gap.h, metrics.height) + line,
            };
            PxRect {
                x: centre(gap.x, gap.w),
                y: top,
                w: line,
                h: bottom - top,
            }
        }
        Axis::Vertical => {
            let left = match rect.origin.col {
                0 => bounds.x,
                _ => centre(gap.x - metrics.width, metrics.width),
            };
            let right = match rect.origin.col + rect.size.cols >= grid.cols {
                true => bounds.x + bounds.w,
                false => centre(gap.x + gap.w, metrics.width) + line,
            };
            PxRect {
                x: left,
                y: centre(gap.y, gap.h),
                w: right - left,
                h: line,
            }
        }
    }
}

pub(crate) fn cell_rect(input: &ChromeInput, rect: CellRect) -> PxRect {
    let origin = grid_origin(
        input.px,
        input.padding,
        input.scale,
        input.zen,
        input.metrics,
    );
    let metrics = input.metrics;
    PxRect {
        x: origin.0 + f32::from(rect.origin.col) * metrics.width,
        y: origin.1 + f32::from(rect.origin.row) * metrics.height,
        w: f32::from(rect.size.cols) * metrics.width,
        h: f32::from(rect.size.rows) * metrics.height,
    }
}

fn intersect(rect: PxRect, clip: PxRect) -> Option<PxRect> {
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let w = (rect.x + rect.w).min(clip.x + clip.w) - x;
    let h = (rect.y + rect.h).min(clip.y + clip.h) - y;
    (w > 0.0 && h > 0.0).then_some(PxRect { x, y, w, h })
}

fn cells(length: f32, cell: f32) -> u16 {
    (length.max(cell) / cell)
        .floor()
        .clamp(1.0, f32::from(u16::MAX)) as u16
}

pub(crate) fn chrome_bottom(scale: f32) -> f32 {
    bar_height(scale) + px(LINE, scale)
}

#[cfg(test)]
#[path = "chrome_layout_tests.rs"]
mod tests;
