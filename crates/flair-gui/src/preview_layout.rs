use flair_core::{
    geom::Axis,
    proto::{Color, SessionPreview, WindowLayout, WindowPreview},
    status::StatusWindow,
    theme::blend,
};

use crate::{
    CellMetrics,
    chrome_bar::{ellipsize, fill, icon_lead, icon_text, opaque, rounded, small_text},
    overlay_layout::{OverlayInput, OverlayScene},
    process_icon::process_icon,
    renderer::{Border, PxRect, Quad},
    ui::{LABEL_HEIGHT, LINE, STRIP_HEIGHT, THUMB_GAP, THUMB_MAX, THUMB_PAD, THUMB_RADIUS, px},
};

const WINDOWS: usize = 9;
const INK: u8 = 55;
const QUAD_CAP: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strip {
    pub windows: Vec<StatusWindow>,
    pub preview: Option<SessionPreview>,
}

pub(crate) fn strip_height(scale: f32) -> f32 {
    px(STRIP_HEIGHT, scale)
}

pub(crate) fn strip(
    strip: &Strip,
    area: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let windows: Vec<&StatusWindow> = strip.windows.iter().take(WINDOWS).collect();
    if windows.is_empty() {
        return;
    }
    let count = windows.len() as f32;
    let gap = px(THUMB_GAP, scale);
    let slot = ((area.w - gap * (count - 1.0)) / count)
        .min(px(THUMB_MAX, scale))
        .floor()
        .max(0.0);
    let label = px(LABEL_HEIGHT, scale);
    let room = (area.h - label).max(0.0);
    let screen = input.px.1 as f32 / input.px.0.max(1) as f32;
    let mut x = area.x.round();
    let mut budget = QUAD_CAP;
    for window in windows {
        let preview = strip
            .preview
            .as_ref()
            .and_then(|preview| preview.windows.iter().find(|item| item.window == window.id));
        let aspect = preview.map_or(screen, |preview| aspect(&preview.layout, input.metrics));
        let width = slot.min((room / aspect.max(0.01)).floor());
        let thumb = PxRect {
            x,
            y: area.y.round(),
            w: width,
            h: (width * aspect).min(room).round(),
        };
        frame(thumb, window.active, input, scene);
        if let Some(preview) = preview {
            budget = minimap(preview, thumb, budget, input, scene);
        }
        caption(window, thumb, label, input, scene, measure);
        scene.hits.windows.push((
            window.id,
            PxRect {
                h: thumb.h + label,
                ..thumb
            },
        ));
        x += width + gap;
    }
}

fn aspect(layout: &WindowLayout, metrics: CellMetrics) -> f32 {
    let size = layout.area.size;
    (f32::from(size.rows.max(1)) * metrics.height) / (f32::from(size.cols.max(1)) * metrics.width)
}

fn frame(thumb: PxRect, current: bool, input: &OverlayInput, scene: &mut OverlayScene) {
    let theme = input.theme;
    let color = match current {
        true => theme.accent,
        false => theme.surfaces().outline,
    };
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(color),
            width: px(LINE, input.scale),
        }),
        ..rounded(thumb, theme.background, THUMB_RADIUS, input.scale)
    });
}

fn minimap(
    preview: &WindowPreview,
    thumb: PxRect,
    budget: usize,
    input: &OverlayInput,
    scene: &mut OverlayScene,
) -> usize {
    let scale = input.scale;
    let theme = input.theme;
    let inset = px(THUMB_PAD, scale);
    let inner = PxRect {
        x: thumb.x + inset,
        y: thumb.y + inset,
        w: (thumb.w - inset * 2.0).max(0.0),
        h: (thumb.h - inset * 2.0).max(0.0),
    };
    let area = preview.layout.area;
    let sx = inner.w / f32::from(area.size.cols.max(1));
    let sy = inner.h / f32::from(area.size.rows.max(1));
    let line = px(LINE, scale);
    let outline = theme.surfaces().outline;
    for separator in &preview.layout.separators {
        let rect = separator.rect;
        let col = f32::from(rect.origin.col.saturating_sub(area.origin.col));
        let row = f32::from(rect.origin.row.saturating_sub(area.origin.row));
        let (cols, rows) = (f32::from(rect.size.cols), f32::from(rect.size.rows));
        let quad = match separator.axis {
            Axis::Horizontal => (
                inner.x + (col + cols / 2.0) * sx,
                inner.y + row * sy,
                line,
                rows * sy,
            ),
            Axis::Vertical => (
                inner.x + col * sx,
                inner.y + (row + rows / 2.0) * sy,
                cols * sx,
                line,
            ),
        };
        scene.quads.push(fill(snap(quad), outline));
    }
    let mut budget = budget;
    for pane in &preview.panes {
        let Some(placement) = preview
            .layout
            .panes
            .iter()
            .find(|placement| placement.pane == pane.pane)
        else {
            continue;
        };
        let left = f32::from(placement.rect.origin.col.saturating_sub(area.origin.col));
        let top = f32::from(placement.rect.origin.row.saturating_sub(area.origin.row));
        for (row, runs) in pane.rows.iter().enumerate() {
            if runs.len() * 2 > budget {
                return 0;
            }
            let y = inner.y + (top + row as f32) * sy;
            for run in runs {
                let x = inner.x + (left + f32::from(run.col)) * sx;
                let w = f32::from(run.len) * sx;
                let bg = theme.resolve(run.bg);
                if run.bg != Color::Background {
                    scene.quads.push(fill(snap((x, y, w, sy)), bg));
                    budget -= 1;
                }
                if let Some(fg) = run.fg {
                    let color = blend(bg, theme.resolve(fg), INK);
                    let quad = (x, y + sy / 4.0, w, (sy / 2.0).max(line));
                    scene.quads.push(fill(snap(quad), color));
                    budget -= 1;
                }
            }
        }
    }
    budget
}

fn snap(quad: (f32, f32, f32, f32)) -> PxRect {
    let (x, y, w, h) = quad;
    let (left, top) = (x.round(), y.round());
    PxRect {
        x: left,
        y: top,
        w: ((x + w).round() - left).max(1.0),
        h: ((y + h).round() - top).max(1.0),
    }
}

fn caption(
    window: &StatusWindow,
    thumb: PxRect,
    height: f32,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let surfaces = input.theme.surfaces();
    let small = input.ui.small;
    let rect = PxRect {
        x: thumb.x,
        y: thumb.y + thumb.h,
        w: thumb.w,
        h: height,
    };
    let number = window.index.to_string();
    let number_width = measure(&number, false) * small;
    let gap = measure(" ", false) * small;
    scene
        .text
        .push(small_text(rect.x, rect, number, surfaces.faint, input.ui));
    let icon = process_icon(&window.program);
    let lead = icon_lead(icon, input.metrics, input.scale);
    let room = (rect.w - number_width - gap - lead).max(0.0);
    let name = ellipsize(&window.name, room / small, false, measure);
    if !name.is_empty() {
        let x = rect.x + number_width + gap;
        let text = small_text((x + lead).round(), rect, name, surfaces.muted, input.ui);
        if let Some(icon) = icon {
            let centre = text.baseline - input.ui.cap / 2.0;
            scene
                .text
                .push(icon_text(x, centre, icon, surfaces.faint, input.metrics));
        }
        scene.text.push(text);
    }
}

#[cfg(test)]
#[path = "preview_layout_tests.rs"]
mod tests;
