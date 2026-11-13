use flair_core::{ids::WindowId, theme::Theme};

use crate::{
    UiMetrics,
    chrome_bar::{baseline, ellipsize, fill, opaque, rounded, small_text, ui_text},
    chrome_layout::chrome_bottom,
    overlay_rows::{entry, head},
    preview_layout::{self, Strip},
    renderer::{Border, PxRect, Quad, TextRun},
    settings_layout::SettingsHit,
    ui::{
        BACKDROP, CAP_HEIGHT, CAP_PAD, CAP_RADIUS, CARET_GAP, DOCK_INSET, DOCK_WIDTH, FIELD_RADIUS,
        FOOTER_GAP, FOOTER_RULE_GAP, ITEM_GAP, ITEM_PAD, LINE, PALETTE_FIELD, PALETTE_GAP,
        PANEL_PAD_X, PANEL_PAD_Y, PANEL_RADIUS, PANEL_TALL, PANEL_TOP, PANEL_WIDTH, ROW_GAP,
        ROW_HEIGHT, ROW_PAD, px,
    },
};

const STRIP_ROWS: usize = 3;

pub struct OverlayInput<'a> {
    pub metrics: crate::CellMetrics,
    pub px: (u32, u32),
    pub scale: f32,
    pub ui: UiMetrics,
    pub theme: &'a Theme,
    pub hover: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Centred,
    Docked,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub depth: u8,
    pub number: Option<u16>,
    pub icon: Option<char>,
    pub name: String,
    pub name_matches: Vec<usize>,
    pub detail: Option<String>,
    pub detail_matches: Vec<usize>,
    pub label: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub text: String,
    pub cursor: usize,
    pub placeholder: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub placement: Placement,
    pub title: Option<&'static str>,
    pub field: Option<Field>,
    pub count: (usize, usize),
    pub rows: Vec<Row>,
    pub selected: usize,
    pub keys: Vec<(&'static str, String)>,
    pub strip: Option<Strip>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OverlayHits {
    pub prompt_scroll_limit: Option<usize>,
    pub panel: PxRect,
    pub rows: Vec<(usize, PxRect)>,
    pub controls: Vec<(SettingsHit, PxRect)>,
    pub windows: Vec<(WindowId, PxRect)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayHit {
    Row(usize),
    Window(WindowId),
    Panel,
    Outside,
    Settings(SettingsHit),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OverlayScene {
    pub quads: Vec<Quad>,
    pub text: Vec<TextRun>,
    pub hits: OverlayHits,
}

impl OverlayHits {
    pub fn hit_test(&self, pos: (f64, f64)) -> OverlayHit {
        let point = (pos.0 as f32, pos.1 as f32);
        if let Some((hit, _)) = self.controls.iter().find(|(_, rect)| inside(*rect, point)) {
            return OverlayHit::Settings(*hit);
        }
        if let Some((index, _)) = self.rows.iter().find(|(_, rect)| inside(*rect, point)) {
            return OverlayHit::Row(*index);
        }
        if let Some((window, _)) = self.windows.iter().find(|(_, rect)| inside(*rect, point)) {
            return OverlayHit::Window(*window);
        }
        if inside(self.panel, point) {
            OverlayHit::Panel
        } else {
            OverlayHit::Outside
        }
    }

    pub fn page(&self) -> usize {
        self.rows.len().max(1)
    }
}

pub fn layout(
    sheet: &Sheet,
    input: &OverlayInput,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> OverlayScene {
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    let mut scene = OverlayScene::default();
    let (panel, (capacity, has_footer, has_head, has_strip)) = match sheet.placement {
        Placement::Centred => centred(sheet, input, &mut scene),
        Placement::Docked => docked(sheet, input),
    };
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: px(LINE, scale),
        }),
        ..rounded(panel, surfaces.surface1, PANEL_RADIUS, scale)
    });
    scene.hits.panel = panel;
    let pad_x = px(PANEL_PAD_X, scale);
    let x = panel.x + pad_x;
    let width = (panel.w - pad_x * 2.0).max(0.0);
    let y = match has_head {
        true => {
            head(sheet, panel, input, &mut scene, measure);
            panel.y + head_top(scale)
        }
        false => panel.y + px(PANEL_PAD_Y, scale),
    };
    let first = sheet.selected.saturating_sub(capacity.saturating_sub(1));
    let shown = sheet.rows.len().saturating_sub(first).min(capacity);
    let pitch = px(ROW_HEIGHT, scale) + px(ROW_GAP, scale);
    let gutters: Vec<u8> = sheet
        .rows
        .iter()
        .filter(|row| row.icon.is_some())
        .map(|row| row.depth)
        .collect();
    for (offset, row) in sheet.rows.iter().skip(first).take(shown).enumerate() {
        let index = first + offset;
        let rect = PxRect {
            x,
            y: y + offset as f32 * pitch,
            w: width,
            h: px(ROW_HEIGHT, scale),
        };
        entry(
            row,
            index == sheet.selected,
            gutters.contains(&row.depth),
            rect,
            input,
            &mut scene,
            measure,
        );
        scene.hits.rows.push((index, rect));
    }
    if sheet.rows.is_empty() && capacity > 0 {
        let rect = PxRect {
            x,
            y,
            w: width,
            h: px(ROW_HEIGHT, scale),
        };
        scene.text.push(ui_text(
            x + px(ROW_PAD, scale),
            baseline(rect, input.ui),
            ellipsize(
                "No matches",
                width - px(ROW_PAD, scale) * 2.0,
                false,
                measure,
            ),
            surfaces.muted,
            false,
        ));
    }
    if !has_footer {
        return scene;
    }
    let bottom = panel.y + panel.h - px(PANEL_PAD_Y, scale) - px(CAP_HEIGHT, scale);
    let rule = bottom - px(FOOTER_RULE_GAP, scale) - px(LINE, scale);
    scene.quads.push(fill(
        PxRect {
            x,
            y: rule,
            w: width,
            h: px(LINE, scale),
        },
        surfaces.outline,
    ));
    if has_strip && let Some(strip) = &sheet.strip {
        let bottom = rule - px(FOOTER_RULE_GAP, scale);
        let top = bottom - preview_layout::strip_height(scale);
        scene.quads.push(fill(
            PxRect {
                x,
                y: top - px(FOOTER_RULE_GAP, scale) - px(LINE, scale),
                w: width,
                h: px(LINE, scale),
            },
            surfaces.outline,
        ));
        preview_layout::strip(
            strip,
            PxRect {
                x,
                y: top,
                w: width,
                h: bottom - top,
            },
            input,
            &mut scene,
            measure,
        );
    }
    footer(
        &sheet.keys,
        sheet.rows.len() - shown,
        input,
        (x, bottom, width),
        &mut scene,
        measure,
    );
    scene
}

fn head_top(scale: f32) -> f32 {
    px(PALETTE_FIELD, scale) + px(LINE, scale) + px(PALETTE_GAP, scale)
}

fn head_height(has_head: bool, scale: f32) -> f32 {
    let top = match has_head {
        true => head_top(scale),
        false => px(PANEL_PAD_Y, scale),
    };
    top + px(PANEL_PAD_Y, scale)
}

fn footer_height(scale: f32) -> f32 {
    px(FOOTER_RULE_GAP, scale) * 2.0 + px(LINE, scale) + px(CAP_HEIGHT, scale)
}

fn strip_block(scale: f32) -> f32 {
    px(FOOTER_RULE_GAP, scale) * 2.0 + px(LINE, scale) + preview_layout::strip_height(scale)
}

fn fit(sheet: &Sheet, room: f32, scale: f32) -> (usize, bool, bool, bool) {
    let has_head =
        (sheet.field.is_some() || sheet.title.is_some()) && head_height(true, scale) <= room;
    let head = head_height(has_head, scale);
    let footer = footer_height(scale);
    let wanted = sheet.rows.len().clamp(1, STRIP_ROWS);
    let block = strip_block(scale);
    match capacity(room - head - footer, scale) {
        0 => (capacity(room - head, scale).min(1), false, has_head, false),
        _ if sheet.strip.is_some() && capacity(room - head - footer - block, scale) >= wanted => (
            capacity(room - head - footer - block, scale),
            true,
            has_head,
            true,
        ),
        rows => (rows, true, has_head, false),
    }
}

fn panel_height(sheet: &Sheet, fit: (usize, bool, bool, bool), scale: f32) -> f32 {
    let (capacity, has_footer, has_head, has_strip) = fit;
    let count = match capacity {
        0 => 0.0,
        _ => sheet.rows.len().clamp(1, capacity) as f32,
    };
    let rows = count * px(ROW_HEIGHT, scale) + (count - 1.0).max(0.0) * px(ROW_GAP, scale);
    let footer = match has_footer {
        true => footer_height(scale),
        false => 0.0,
    };
    let strip = match has_strip {
        true => strip_block(scale),
        false => 0.0,
    };
    head_height(has_head, scale) + rows + strip + footer
}

fn capacity(room: f32, scale: f32) -> usize {
    let pitch = px(ROW_HEIGHT, scale) + px(ROW_GAP, scale);
    ((room + px(ROW_GAP, scale)) / pitch).floor().max(0.0) as usize
}

fn centred(
    sheet: &Sheet,
    input: &OverlayInput,
    scene: &mut OverlayScene,
) -> (PxRect, (usize, bool, bool, bool)) {
    let scale = input.scale;
    let (width, height) = (input.px.0 as f32, input.px.1 as f32);
    let below = chrome_bottom(scale);
    scene.quads.push(Quad {
        rect: PxRect {
            x: 0.0,
            y: below,
            w: width,
            h: (height - below).max(0.0),
        },
        color: BACKDROP,
        radius: 0.0,
        border: None,
    });
    let margin = px(PANEL_PAD_Y, scale);
    let tall = height >= px(PANEL_TALL, scale);
    let (top, room) = match tall {
        true => (px(PANEL_TOP, scale), height - px(PANEL_TOP, scale) - margin),
        false => (below + margin, height - below - margin * 2.0),
    };
    let fit = fit(sheet, room, scale);
    let panel_height = panel_height(sheet, fit, scale).min(room.max(0.0));
    let panel_width = px(PANEL_WIDTH, scale)
        .min(width - px(PANEL_PAD_X, scale) * 2.0)
        .max(0.0)
        .round();
    let y = match tall {
        true => top,
        false => top + ((room - panel_height) / 2.0).max(0.0).floor(),
    };
    (
        PxRect {
            x: ((width - panel_width) / 2.0).round(),
            y,
            w: panel_width,
            h: panel_height,
        },
        fit,
    )
}

fn docked(sheet: &Sheet, input: &OverlayInput) -> (PxRect, (usize, bool, bool, bool)) {
    let scale = input.scale;
    let (width, height) = (input.px.0 as f32, input.px.1 as f32);
    let inset = px(DOCK_INSET, scale);
    let panel_width = px(DOCK_WIDTH, scale)
        .min(width - inset * 2.0)
        .max(0.0)
        .round();
    let y = chrome_bottom(scale) + inset;
    let panel = PxRect {
        x: (width - inset - panel_width).round(),
        y,
        w: panel_width,
        h: (height - inset - y).max(0.0),
    };
    (panel, fit(sheet, panel.h, scale))
}

pub(crate) fn search(
    field: &Field,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let line = px(LINE, scale);
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: line,
        }),
        ..rounded(rect, theme.background, FIELD_RADIUS, scale)
    });
    let pad = px(ITEM_PAD, scale);
    let text_x = rect.x + pad;
    let room = (rect.w - pad * 2.0 - line).max(0.0);
    let (text, offset, caret, color) = match field.text.is_empty() {
        true => {
            let gap = px(CARET_GAP, scale);
            (
                ellipsize(field.placeholder, room - gap, false, measure),
                gap,
                0.0,
                surfaces.faint,
            )
        }
        false => {
            let (text, caret) = visible(&field.text, field.cursor, room, measure);
            (text, 0.0, caret, theme.foreground)
        }
    };
    scene.text.push(ui_text(
        text_x + offset,
        baseline(rect, input.ui),
        text,
        color,
        false,
    ));
    scene.quads.push(fill(
        PxRect {
            x: (text_x + caret).round(),
            y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
            w: line,
            h: input.ui.height.round(),
        },
        theme.accent,
    ));
}

pub(crate) fn visible(
    text: &str,
    cursor: usize,
    room: f32,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> (String, f32) {
    let widths: Vec<f32> = text
        .chars()
        .map(|c| measure(c.encode_utf8(&mut [0; 4]), false))
        .collect();
    let cursor = cursor.min(widths.len());
    let mut start = 0;
    while start < cursor && widths[start..cursor].iter().sum::<f32>() > room {
        start += 1;
    }
    let mut used = 0.0;
    let count = widths[start..]
        .iter()
        .take_while(|width| {
            used += *width;
            used <= room
        })
        .count();
    (
        text.chars().skip(start).take(count).collect(),
        widths[start..cursor].iter().sum(),
    )
}

pub(crate) fn cap(
    text: &str,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (x, row) = at;
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    let height = px(CAP_HEIGHT, scale);
    let text_width = measure(text, false) * input.ui.small;
    let width = (text_width + px(CAP_PAD, scale) * 2.0).max(height).round();
    let rect = PxRect {
        x: x.round(),
        y: (row.y + (row.h - height) / 2.0).round(),
        w: width,
        h: height,
    };
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline_strong),
            width: px(LINE, scale),
        }),
        ..rounded(rect, surfaces.surface3, CAP_RADIUS, scale)
    });
    scene.text.push(small_text(
        (rect.x + (width - text_width) / 2.0).round(),
        rect,
        text.to_owned(),
        input.theme.foreground,
        input.ui,
    ));
    width
}

pub(crate) fn footer(
    keys: &[(&'static str, String)],
    hidden: usize,
    input: &OverlayInput,
    area: (f32, f32, f32),
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (left, y, width) = area;
    let scale = input.scale;
    let muted = input.theme.surfaces().muted;
    let rect = PxRect {
        x: left,
        y,
        w: width,
        h: px(CAP_HEIGHT, scale),
    };
    let mut end = left + width;
    if hidden > 0 {
        let more = format!("+{hidden} more");
        let more_width = measure(&more, false) * input.ui.small;
        scene.text.push(small_text(
            (end - more_width).round(),
            rect,
            more,
            muted,
            input.ui,
        ));
        end -= more_width + px(FOOTER_GAP, scale);
    }
    let gap = px(ITEM_GAP, scale);
    let mut x = left;
    for (index, (key, verb)) in keys.iter().enumerate() {
        let cap_width = (measure(key, false) * input.ui.small + px(CAP_PAD, scale) * 2.0)
            .max(px(CAP_HEIGHT, scale))
            .round();
        let room = end - x - cap_width - gap;
        let verb = match measure(verb, false) <= room {
            true => verb.clone(),
            false if index == 0 => ellipsize(verb, room, false, measure),
            false => break,
        };
        if verb.is_empty() {
            break;
        }
        x += cap(key, (x, rect), input, scene, measure) + gap;
        let verb_width = measure(&verb, false);
        scene.text.push(ui_text(
            x.round(),
            baseline(rect, input.ui),
            verb,
            muted,
            false,
        ));
        x += verb_width + px(FOOTER_GAP, scale);
    }
}

pub(crate) fn inside(rect: PxRect, point: (f32, f32)) -> bool {
    point.0 >= rect.x && point.0 < rect.x + rect.w && point.1 >= rect.y && point.1 < rect.y + rect.h
}

#[cfg(test)]
#[path = "overlay_layout_tests.rs"]
mod tests;
