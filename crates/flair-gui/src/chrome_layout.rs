use flair_core::{
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::WindowId,
    proto::{Separator, WindowLayout},
    theme::Theme,
};

use crate::{
    CellMetrics, UiMetrics,
    chrome_bar::{self, Scene, fill, opaque, rounded},
    renderer::{Border, Chrome, PxRect, Quad},
    status::StatusParts,
    ui::{BACKDROP, BAR, DOCK_INSET, DOCK_WIDTH, LINE, PANEL_PAD_X, PANEL_PAD_Y, PANEL_RADIUS, px},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarField<'a> {
    pub text: &'a str,
    pub cursor: usize,
}

pub struct ChromeInput<'a> {
    pub px: (u32, u32),
    pub scale: f32,
    pub padding: (f32, f32),
    pub metrics: CellMetrics,
    pub ui: UiMetrics,
    pub theme: &'a Theme,
    pub status: &'a StatusParts,
    pub prompt: Option<BarField<'a>>,
    pub layout: Option<&'a WindowLayout>,
    pub hover: Option<(f32, f32)>,
    pub modal: Option<Modal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Centered(CellRect),
    Docked(CellRect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Window(WindowId),
    Session,
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
}

pub fn bar_height(scale: f32) -> f32 {
    px(BAR, scale)
}

pub fn grid_origin(
    px: (u32, u32),
    padding: (f32, f32),
    scale: f32,
    metrics: CellMetrics,
) -> (f32, f32) {
    let size = grid_size(px, padding, scale, metrics);
    let top = chrome_bottom(scale) + padding.1;
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
    metrics: CellMetrics,
) -> GridSize {
    let width = px.0 as f32 - padding.0 * 2.0;
    let height = px.1 as f32 - chrome_bottom(scale) - padding.1 * 2.0;
    GridSize {
        cols: cells(width, metrics.width),
        rows: cells(height, metrics.height),
    }
}

pub fn modal_area(size: GridSize, metrics: CellMetrics, scale: f32) -> CellRect {
    let margin = px(PANEL_PAD_X, scale);
    let col = ((margin / metrics.width).ceil() as u16).min(size.cols / 2);
    let row = ((margin / metrics.height).ceil() as u16).min(size.rows / 2);
    CellRect {
        origin: CellPoint { col, row },
        size: GridSize {
            cols: size.cols - col * 2,
            rows: size.rows - row * 2,
        },
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
    chrome_bar::bar(input, &mut scene, &mut measure);
    let origin = grid_origin(input.px, input.padding, scale, input.metrics);
    let top = chrome_bottom(scale) + input.padding.1;
    let clip = PxRect {
        x: input.padding.0,
        y: top,
        w: (width - input.padding.0 * 2.0).max(0.0),
        h: (height - top - input.padding.1).max(0.0),
    };
    if let Some(window) = input.layout {
        separators(input, window, clip, &mut scene);
    }
    let (modal, lifted) = match input.modal {
        Some(Modal::Centered(rect)) => (centered_modal(input, rect), Some(rect)),
        Some(Modal::Docked(rect)) => (docked_modal(input, rect), Some(rect)),
        None => (Vec::new(), None),
    };
    ChromeLayout {
        chrome: Chrome {
            scale,
            origin,
            clip,
            extend: PxRect {
                x: 0.0,
                y: chrome_bottom(scale),
                w: width,
                h: (height - chrome_bottom(scale)).max(0.0),
            },
            quads: scene.quads,
            text: scene.text,
            modal,
            lifted,
        },
        hits: scene.hits,
    }
}

fn separators(input: &ChromeInput, window: &WindowLayout, clip: PxRect, scene: &mut Scene) {
    let theme = input.theme;
    let active = window
        .panes
        .iter()
        .find(|placement| placement.pane == window.active)
        .map(|placement| placement.rect);
    for separator in &window.separators {
        if let Some(rect) = intersect(separator_rect(input, separator.axis, separator.rect), clip) {
            scene.quads.push(fill(rect, theme.surfaces().outline));
        }
        if let Some(rect) = active
            .and_then(|pane| adjacent(separator, pane))
            .and_then(|segment| intersect(separator_rect(input, separator.axis, segment), clip))
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

fn separator_rect(input: &ChromeInput, axis: Axis, rect: CellRect) -> PxRect {
    let gap = cell_rect(input, rect);
    let line = px(LINE, input.scale);
    match axis {
        Axis::Horizontal => PxRect {
            x: (gap.x + (gap.w - line) / 2.0).round(),
            y: gap.y.round(),
            w: line,
            h: gap.h.round(),
        },
        Axis::Vertical => PxRect {
            x: gap.x.round(),
            y: (gap.y + (gap.h - line) / 2.0).round(),
            w: gap.w.round(),
            h: line,
        },
    }
}

fn panel(rect: PxRect, input: &ChromeInput) -> Quad {
    let surfaces = input.theme.surfaces();
    Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: px(LINE, input.scale),
        }),
        ..rounded(rect, surfaces.surface1, PANEL_RADIUS, input.scale)
    }
}

fn centered_modal(input: &ChromeInput, lifted: CellRect) -> Vec<Quad> {
    let inner = cell_rect(input, lifted);
    let pad_x = px(PANEL_PAD_X, input.scale);
    let pad_y = px(PANEL_PAD_Y, input.scale);
    let frame = panel(
        PxRect {
            x: (inner.x - pad_x).round(),
            y: (inner.y - pad_y).round(),
            w: (inner.w + pad_x * 2.0).round(),
            h: (inner.h + pad_y * 2.0).round(),
        },
        input,
    );
    let top = chrome_bottom(input.scale);
    let backdrop = Quad {
        rect: PxRect {
            x: 0.0,
            y: top,
            w: input.px.0 as f32,
            h: (input.px.1 as f32 - top).max(0.0),
        },
        color: BACKDROP,
        radius: 0.0,
        border: None,
    };
    vec![backdrop, frame]
}

fn docked_modal(input: &ChromeInput, lifted: CellRect) -> Vec<Quad> {
    let inset = px(DOCK_INSET, input.scale);
    let width = input.px.0 as f32;
    let right = width - inset;
    let x = (right - px(DOCK_WIDTH, input.scale))
        .min(cell_rect(input, lifted).x - inset)
        .max(0.0)
        .round();
    let y = chrome_bottom(input.scale) + inset;
    let rect = PxRect {
        x,
        y,
        w: (right - x).max(0.0),
        h: (input.px.1 as f32 - inset - y).max(0.0),
    };
    vec![panel(rect, input)]
}

fn cell_rect(input: &ChromeInput, rect: CellRect) -> PxRect {
    let origin = grid_origin(input.px, input.padding, input.scale, input.metrics);
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

fn chrome_bottom(scale: f32) -> f32 {
    bar_height(scale) + px(LINE, scale)
}

#[cfg(test)]
#[path = "chrome_layout_tests.rs"]
mod tests;
