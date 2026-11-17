use flair_core::{ids::WindowId, theme::Theme};

use crate::{
    UiMetrics,
    chrome_bar::{baseline, ellipsize, opaque, rounded, small_text, ui_text},
    palette_layout,
    preview_layout::Strip,
    renderer::{Border, PxRect, Quad, TextRun},
    settings_layout::SettingsHit,
    t3_style::{
        PANEL_RADIUS as T3_PANEL_RADIUS, SHADOW, SHADOW_DROP, SHADOW_LAYERS, SHADOW_SPREAD,
    },
    ui::{CAP_HEIGHT, CAP_PAD, CAP_RADIUS, FOOTER_GAP, ITEM_GAP, LINE, px},
    ui::{CHEVRON_DOWN, CHEVRON_RIGHT},
};

pub struct OverlayInput<'a> {
    pub metrics: crate::CellMetrics,
    pub px: (u32, u32),
    pub scale: f32,
    pub ui: UiMetrics,
    pub theme: &'a Theme,
    pub hover: Option<(f32, f32)>,
    pub icons: bool,
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
    pub current: bool,
    pub monogram: bool,
    pub muted: bool,
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
    pub empty: Option<&'static str>,
    pub glyph: char,
}

impl Sheet {
    pub fn without_icons(self) -> Sheet {
        Sheet {
            rows: self
                .rows
                .into_iter()
                .map(|row| Row {
                    icon: row
                        .icon
                        .filter(|icon| [CHEVRON_RIGHT, CHEVRON_DOWN].contains(icon)),
                    ..row
                })
                .collect(),
            ..self
        }
    }
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
    palette_layout::layout(sheet, input, measure)
}

pub(crate) fn shadow(panel: PxRect, window: PxRect, scale: f32, scene: &mut OverlayScene) {
    let drop = px(SHADOW_DROP, scale);
    for layer in (1..=SHADOW_LAYERS).rev() {
        let spread = px(SHADOW_SPREAD, scale) * layer as f32;
        let rect = PxRect {
            x: panel.x - spread,
            y: panel.y + drop - spread,
            w: panel.w + spread * 2.0,
            h: panel.h + spread * 2.0,
        };
        let fits = rect.x >= window.x
            && rect.y >= window.y
            && rect.x + rect.w <= window.x + window.w
            && rect.y + rect.h <= window.y + window.h;
        if fits {
            scene.quads.push(Quad {
                rect,
                color: SHADOW,
                radius: px(T3_PANEL_RADIUS, scale) + spread,
                border: None,
            });
        }
    }
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
