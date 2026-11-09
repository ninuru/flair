use flair_core::{
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::WindowId,
    proto::Separator,
    theme::{Rgb, Rgba, Theme},
};

use crate::{
    CellMetrics, UiMetrics,
    renderer::{Border, Chrome, PxRect, Quad, TextRun},
    status::{BadgeKind, StatusParts, TabParts},
};

const BAR: f32 = 34.0;
const LINE: f32 = 1.0;
const INSET: f32 = 8.0;
const RADIUS: f32 = 4.0;
const CHIP_HEIGHT: f32 = 22.0;
const CHIP_PAD: f32 = 6.0;
const CHIP_GAP: f32 = 4.0;
const CHIP_NAME_MAX: f32 = 180.0;
const DOT: f32 = 6.0;
const TAB_HEIGHT: f32 = 26.0;
const TAB_PAD: f32 = 10.0;
const TAB_GAP: f32 = 2.0;
const TAB_NAME_MAX: f32 = 160.0;
const BADGE_HEIGHT: f32 = 22.0;
const BADGE_PAD: f32 = 8.0;
const BADGE_GAP: f32 = 6.0;
const MODAL_MARGIN: f32 = 24.0;
const MODAL_RADIUS: f32 = 8.0;
const BACKDROP: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 179,
};
const ELLIPSIS: &str = "…";

pub struct ChromeInput<'a> {
    pub px: (u32, u32),
    pub scale: f32,
    pub padding: (f32, f32),
    pub metrics: CellMetrics,
    pub ui: UiMetrics,
    pub theme: &'a Theme,
    pub status: &'a StatusParts,
    pub prompt: Option<&'a str>,
    pub separators: &'a [Separator],
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

enum ChipItem {
    Text {
        text: String,
        color: Rgb,
        bold: bool,
    },
    Dot,
}

#[derive(Default)]
struct Scene {
    quads: Vec<Quad>,
    text: Vec<TextRun>,
    hits: Vec<Hit>,
}

pub fn bar_height(scale: f32) -> f32 {
    px(BAR, scale)
}

pub fn grid_origin(padding: (f32, f32), scale: f32) -> (f32, f32) {
    (padding.0, chrome_bottom(scale) + padding.1)
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
    let margin = px(MODAL_MARGIN, scale);
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
    let surfaces = input.theme.surfaces();
    let bar = bar_height(scale);
    let mut scene = Scene::default();
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: bar,
        },
        surfaces.surface1,
    ));
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: bar,
            w: width,
            h: px(LINE, scale),
        },
        surfaces.outline,
    ));
    let start = chip(input, &mut scene, &mut measure) + px(INSET, scale);
    let end = badges(input, &mut scene, &mut measure) - px(INSET, scale);
    match input.prompt {
        Some(prompt) => prompt_line(input, prompt, start, end, &mut scene, &mut measure),
        None => tabs(input, start, end, &mut scene, &mut measure),
    }
    let origin = grid_origin(input.padding, scale);
    let clip = PxRect {
        x: origin.0,
        y: origin.1,
        w: (width - input.padding.0 * 2.0).max(0.0),
        h: (height - origin.1 - input.padding.1).max(0.0),
    };
    for separator in input.separators {
        if let Some(rect) = intersect(separator_rect(input, separator), clip) {
            scene.quads.push(fill(rect, surfaces.pane_border));
        }
    }
    let (modal, lifted) = match input.modal {
        Some(Modal::Centered(rect)) => (centered_modal(input, rect), Some(rect)),
        Some(Modal::Docked(rect)) => (docked_modal(input, rect), Some(rect)),
        None => (Vec::new(), None),
    };
    ChromeLayout {
        chrome: Chrome {
            origin,
            clip,
            quads: scene.quads,
            text: scene.text,
            modal,
            lifted,
        },
        hits: scene.hits,
    }
}

fn chip(
    input: &ChromeInput,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let chip = &input.status.chip;
    let mut items = vec![ChipItem::Text {
        text: ellipsize(&chip.session, px(CHIP_NAME_MAX, scale), true, measure),
        color: theme.foreground,
        bold: true,
    }];
    if chip.hidden > 0 {
        items.push(ChipItem::Text {
            text: format!("+{}", chip.hidden),
            color: surfaces.muted,
            bold: false,
        });
    }
    if chip.running {
        items.push(ChipItem::Dot);
    }
    if chip.finished {
        items.push(ChipItem::Text {
            text: "!".into(),
            color: theme.accent,
            bold: true,
        });
    }
    let widths: Vec<f32> = items
        .iter()
        .map(|item| match item {
            ChipItem::Text { text, bold, .. } => measure(text, *bold),
            ChipItem::Dot => px(DOT, scale),
        })
        .collect();
    let pad = px(CHIP_PAD, scale);
    let gap = px(CHIP_GAP, scale);
    let width = (pad * 2.0 + span(&widths, gap)).round();
    let rect = bar_item(px(INSET, scale), width, px(CHIP_HEIGHT, scale), scale);
    scene
        .quads
        .push(bordered(rect, surfaces.surface2, surfaces.outline, scale));
    let mut x = rect.x + pad;
    for (item, item_width) in items.into_iter().zip(widths) {
        match item {
            ChipItem::Text { text, color, bold } => scene.text.push(TextRun {
                x,
                baseline: baseline(rect, input.ui),
                text,
                color,
                bold,
            }),
            ChipItem::Dot => scene.quads.push(Quad {
                rect: PxRect {
                    x,
                    y: (rect.y + (rect.h - item_width) / 2.0).round(),
                    w: item_width,
                    h: item_width,
                },
                color: opaque(theme.accent),
                radius: item_width / 2.0,
                border: None,
            }),
        }
        x += item_width + gap;
    }
    scene.hits.push(Hit {
        rect,
        target: HitTarget::Session,
    });
    rect.x + rect.w
}

fn badges(
    input: &ChromeInput,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let pad = px(BADGE_PAD, scale);
    let mut right = input.px.0 as f32 - px(INSET, scale);
    let mut left = right + px(INSET, scale);
    for badge in &input.status.badges {
        let (fill_color, border_color, text_color) = match badge.kind {
            BadgeKind::Copy => (surfaces.accent_soft, theme.accent, theme.accent),
            BadgeKind::Prefix => (surfaces.surface3, surfaces.outline, theme.foreground),
        };
        let width = (measure(&badge.text, true) + pad * 2.0).round();
        let rect = bar_item(right - width, width, px(BADGE_HEIGHT, scale), scale);
        scene
            .quads
            .push(bordered(rect, fill_color, border_color, scale));
        scene.text.push(TextRun {
            x: rect.x + pad,
            baseline: baseline(rect, input.ui),
            text: badge.text.clone(),
            color: text_color,
            bold: true,
        });
        left = rect.x;
        right = rect.x - px(BADGE_GAP, scale);
    }
    left
}

fn tabs(
    input: &ChromeInput,
    start: f32,
    end: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let tabs = &input.status.tabs;
    let pad = px(TAB_PAD, scale);
    let gap = px(TAB_GAP, scale);
    let labels: Vec<String> = tabs
        .iter()
        .map(|tab| {
            let name = ellipsize(&tab.name, px(TAB_NAME_MAX, scale), false, measure);
            format!("{} {name}", tab.index)
        })
        .collect();
    let widths: Vec<f32> = labels
        .iter()
        .map(|label| (measure(label, false) + pad * 2.0).round())
        .collect();
    let active = tabs.iter().position(|tab| tab.active).unwrap_or(0);
    let mut first = 0;
    while first < active && span(&widths[first..=active], gap) > end - start {
        first += 1;
    }
    let mut x = start;
    for ((tab, label), width) in tabs.iter().zip(labels).zip(widths).skip(first) {
        let (label, width) = if x + width <= end {
            (label, width)
        } else if x == start {
            let label = ellipsize(&label, end - start - pad * 2.0, false, measure);
            let width = (measure(&label, false) + pad * 2.0).round();
            (label, width)
        } else {
            break;
        };
        if label.is_empty() {
            break;
        }
        let rect = bar_item(x, width, px(TAB_HEIGHT, scale), scale);
        draw_tab(input, tab, rect, label, scene);
        x += width + gap;
    }
}

fn draw_tab(input: &ChromeInput, tab: &TabParts, rect: PxRect, label: String, scene: &mut Scene) {
    let surfaces = input.theme.surfaces();
    if tab.active {
        scene.quads.push(Quad {
            rect,
            color: opaque(surfaces.surface3),
            radius: px(RADIUS, input.scale),
            border: None,
        });
    }
    scene.text.push(TextRun {
        x: rect.x + px(TAB_PAD, input.scale),
        baseline: baseline(rect, input.ui),
        text: label,
        color: if tab.active {
            input.theme.foreground
        } else {
            surfaces.muted
        },
        bold: false,
    });
    scene.hits.push(Hit {
        rect,
        target: HitTarget::Window(tab.window),
    });
}

fn prompt_line(
    input: &ChromeInput,
    prompt: &str,
    start: f32,
    end: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let bar = bar_height(input.scale);
    let line = px(LINE, input.scale);
    let text = tail(prompt, end - start, false, measure);
    let caret = (start + measure(&text, false)).min(end - line).round();
    let rect = PxRect {
        x: start,
        y: 0.0,
        w: 0.0,
        h: bar,
    };
    scene.text.push(TextRun {
        x: start,
        baseline: baseline(rect, input.ui),
        text,
        color: input.theme.foreground,
        bold: false,
    });
    scene.quads.push(fill(
        PxRect {
            x: caret,
            y: ((bar - input.ui.height) / 2.0).round(),
            w: line,
            h: input.ui.height,
        },
        input.theme.accent,
    ));
}

fn separator_rect(input: &ChromeInput, separator: &Separator) -> PxRect {
    let gap = cell_rect(input, separator.rect);
    let line = px(LINE, input.scale);
    match separator.axis {
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

fn centered_modal(input: &ChromeInput, lifted: CellRect) -> Vec<Quad> {
    let surfaces = input.theme.surfaces();
    let inner = cell_rect(input, lifted);
    let margin = px(MODAL_MARGIN, input.scale);
    let frame = Quad {
        radius: px(MODAL_RADIUS, input.scale),
        ..bordered(
            PxRect {
                x: (inner.x - margin).round(),
                y: (inner.y - margin).round(),
                w: (inner.w + margin * 2.0).round(),
                h: (inner.h + margin * 2.0).round(),
            },
            surfaces.surface1,
            surfaces.outline,
            input.scale,
        )
    };
    let backdrop = Quad {
        rect: PxRect {
            x: 0.0,
            y: 0.0,
            w: input.px.0 as f32,
            h: input.px.1 as f32,
        },
        color: BACKDROP,
        radius: 0.0,
        border: None,
    };
    vec![backdrop, frame]
}

fn docked_modal(input: &ChromeInput, lifted: CellRect) -> Vec<Quad> {
    let surfaces = input.theme.surfaces();
    let x = (cell_rect(input, lifted).x - px(MODAL_MARGIN, input.scale)).round();
    let y = chrome_bottom(input.scale);
    let h = (input.px.1 as f32 - y).max(0.0);
    let panel = PxRect {
        x,
        y,
        w: (input.px.0 as f32 - x).max(0.0),
        h,
    };
    let line = PxRect {
        x,
        y,
        w: px(LINE, input.scale),
        h,
    };
    vec![fill(panel, surfaces.surface1), fill(line, surfaces.outline)]
}

fn cell_rect(input: &ChromeInput, rect: CellRect) -> PxRect {
    let origin = grid_origin(input.padding, input.scale);
    let metrics = input.metrics;
    PxRect {
        x: origin.0 + f32::from(rect.origin.col) * metrics.width,
        y: origin.1 + f32::from(rect.origin.row) * metrics.height,
        w: f32::from(rect.size.cols) * metrics.width,
        h: f32::from(rect.size.rows) * metrics.height,
    }
}

fn bar_item(x: f32, width: f32, height: f32, scale: f32) -> PxRect {
    PxRect {
        x,
        y: ((bar_height(scale) - height) / 2.0).round(),
        w: width,
        h: height,
    }
}

fn baseline(rect: PxRect, ui: UiMetrics) -> f32 {
    (rect.y + (rect.h - ui.height) / 2.0 + ui.ascent).round()
}

fn ellipsize(
    text: &str,
    max: f32,
    bold: bool,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> String {
    let widths = advances(text, bold, measure);
    if widths.iter().sum::<f32>() <= max {
        return text.to_owned();
    }
    let ellipsis = measure(ELLIPSIS, bold);
    if ellipsis > max {
        return String::new();
    }
    let mut prefix = 0.0;
    let count = widths
        .iter()
        .take_while(|width| {
            prefix += *width;
            prefix + ellipsis <= max
        })
        .count();
    text.chars().take(count).collect::<String>() + ELLIPSIS
}

fn tail(text: &str, max: f32, bold: bool, measure: &mut impl FnMut(&str, bool) -> f32) -> String {
    let widths = advances(text, bold, measure);
    if widths.iter().sum::<f32>() <= max {
        return text.to_owned();
    }
    let ellipsis = measure(ELLIPSIS, bold);
    if ellipsis > max {
        return String::new();
    }
    let mut used = ellipsis;
    let count = widths
        .iter()
        .rev()
        .take_while(|width| {
            used += *width;
            used <= max
        })
        .count();
    ELLIPSIS
        .chars()
        .chain(text.chars().skip(widths.len() - count))
        .collect()
}

fn advances(text: &str, bold: bool, measure: &mut impl FnMut(&str, bool) -> f32) -> Vec<f32> {
    text.chars()
        .map(|c| measure(c.encode_utf8(&mut [0; 4]), bold))
        .collect()
}

fn intersect(rect: PxRect, clip: PxRect) -> Option<PxRect> {
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let w = (rect.x + rect.w).min(clip.x + clip.w) - x;
    let h = (rect.y + rect.h).min(clip.y + clip.h) - y;
    (w > 0.0 && h > 0.0).then_some(PxRect { x, y, w, h })
}

fn span(widths: &[f32], gap: f32) -> f32 {
    widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32
}

fn cells(length: f32, cell: f32) -> u16 {
    (length.max(cell) / cell)
        .floor()
        .clamp(1.0, f32::from(u16::MAX)) as u16
}

fn chrome_bottom(scale: f32) -> f32 {
    bar_height(scale) + px(LINE, scale)
}

fn px(value: f32, scale: f32) -> f32 {
    (value * scale).round()
}

fn fill(rect: PxRect, color: Rgb) -> Quad {
    Quad {
        rect,
        color: opaque(color),
        radius: 0.0,
        border: None,
    }
}

fn bordered(rect: PxRect, fill_color: Rgb, border_color: Rgb, scale: f32) -> Quad {
    Quad {
        rect,
        color: opaque(fill_color),
        radius: px(RADIUS, scale),
        border: Some(Border {
            color: opaque(border_color),
            width: px(LINE, scale),
        }),
    }
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

#[cfg(test)]
#[path = "chrome_layout_tests.rs"]
mod tests;
