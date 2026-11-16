use flair_core::theme::Rgb;

use super::*;
use crate::{
    chrome_bar::small_text,
    overlay_layout::visible,
    t3_style::{
        APPEARANCE_ICON, FONT_ICON, ICON, ICON_BOX, ICON_GAP, KBD, KBD_HEIGHT, KBD_PAD, KBD_RADIUS,
        KEYS_ICON, NAV_CURRENT, NAV_GROUP_GAP, NAV_HEADING, NAV_HEADING_GAP, NAV_HOVER, NAV_ITEM,
        NAV_ITEM_GAP, NAV_ITEM_PAD, NAV_ITEM_RADIUS, PANELS_ICON, PRIMARY, PROMPT_ICON, QUIET,
        SEARCH_ICON, SEARCH_KEY, SEARCH_RADIUS, SIDEBAR, SIDEBAR_BORDER, SIDEBAR_PAD,
        TERMINAL_ICON, WINDOW_ICON,
    },
};

const SEARCH: &str = "Search";

pub(super) fn layout(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let rail = match frame.nav {
        Nav::Sidebar => false,
        Nav::Rail => true,
        Nav::Hidden => return,
    };
    let scale = input.scale;
    let panel = frame.panel;
    let line = px(LINE, scale);
    let width = nav_width(frame.nav, scale);
    let inner = PxRect {
        x: panel.x + line,
        y: panel.y + line,
        w: width - line,
        h: (panel.h - line * 2.0).max(0.0),
    };
    let radius = px(PANEL_RADIUS, scale) - line;
    scene.quads.push(Quad {
        rect: inner,
        color: opaque(SIDEBAR),
        radius,
        border: None,
    });
    scene.quads.push(fill(
        PxRect {
            x: inner.x + radius,
            w: (inner.w - radius).max(0.0),
            ..inner
        },
        SIDEBAR,
    ));
    scene.quads.push(fill(
        PxRect {
            x: panel.x + width,
            w: line,
            ..inner
        },
        SIDEBAR_BORDER,
    ));
    let pad = px(SIDEBAR_PAD, scale);
    let row = PxRect {
        x: panel.x + pad,
        y: panel.y + pad,
        w: width - pad * 2.0,
        h: px(NAV_ITEM, scale),
    };
    let bottom = panel.y + panel.h - pad;
    if row.y + row.h > bottom {
        return;
    }
    search(sheet, row, rail, input, scene, measure);
    let mut y = row.y + row.h + pad;
    let mut previous: Option<Option<&str>> = None;
    for (index, section) in sheet.sections.iter().enumerate() {
        let group = group(*section);
        if previous.is_some_and(|last| last != group) {
            y += px(NAV_GROUP_GAP, scale) - px(NAV_ITEM_GAP, scale);
            if let (Some(heading), false) = (group, rail) {
                let rect = PxRect {
                    y,
                    h: px(NAV_HEADING, scale),
                    ..row
                };
                if rect.y + rect.h > bottom {
                    return;
                }
                scene.text.push(small_text(
                    rect.x + px(SIDEBAR_PAD, scale),
                    rect,
                    ellipsize(heading, rect.w / input.ui.small, false, measure),
                    QUIET,
                    input.ui,
                ));
                y += rect.h + px(NAV_HEADING_GAP, scale);
            }
        }
        previous = Some(group);
        let rect = PxRect { y, ..row };
        if rect.y + rect.h > bottom {
            return;
        }
        let current = sheet.section == Some(index);
        item(
            *section,
            (current && sheet.sections_focused, current),
            (rect, rail),
            input,
            scene,
            measure,
        );
        scene
            .hits
            .controls
            .push((SettingsHit::Section(index), rect));
        y += rect.h + px(NAV_ITEM_GAP, scale);
    }
}

fn search(
    sheet: &SettingsSheet,
    row: PxRect,
    rail: bool,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let field = &sheet.search;
    let typed = !field.text.is_empty();
    let hovered = input.hover.is_some_and(|point| inside(row, point));
    if typed || hovered {
        scene
            .quads
            .push(rounded(row, NAV_HOVER, SEARCH_RADIUS, scale));
    }
    scene.hits.controls.push((SettingsHit::Search, row));
    let pad = px(SIDEBAR_PAD, scale);
    let size = px(ICON_BOX, scale);
    let icon_box = PxRect {
        x: match rail {
            true => row.x + ((row.w - size) / 2.0).round(),
            false => row.x + pad,
        },
        y: row.y + ((row.h - size) / 2.0).round(),
        w: size,
        h: size,
    };
    scene
        .text
        .push(glyph(SEARCH_ICON, icon_box, QUIET, input.metrics));
    if rail {
        return;
    }
    let x = icon_box.x + size + px(ICON_GAP, scale);
    let end = row.x + row.w - pad;
    let base = baseline(row, input.ui);
    if typed {
        let (text, caret) = visible(&field.text, field.cursor, end - x, measure);
        scene.text.push(ui_text(x, base, text, FOREGROUND, false));
        scene.quads.push(fill(
            PxRect {
                x: (x + caret).round(),
                y: (row.y + (row.h - input.ui.height) / 2.0).round(),
                w: px(LINE, scale),
                h: input.ui.height.round(),
            },
            FOREGROUND,
        ));
        return;
    }
    let height = px(KBD_HEIGHT, scale);
    let key_width = measure(SEARCH_KEY, false) * input.ui.small;
    let cap = PxRect {
        w: (key_width + px(KBD_PAD, scale) * 2.0).max(height).round(),
        h: height,
        x: 0.0,
        y: row.y + ((row.h - height) / 2.0).round(),
    };
    let cap = PxRect {
        x: (end - cap.w).round(),
        ..cap
    };
    scene.quads.push(rounded(cap, KBD, KBD_RADIUS, scale));
    scene.text.push(small_text(
        (cap.x + (cap.w - key_width) / 2.0).round(),
        cap,
        SEARCH_KEY.to_owned(),
        MUTED,
        input.ui,
    ));
    scene.text.push(ui_text(
        x,
        base,
        ellipsize(SEARCH, cap.x - x - pad, false, measure),
        MUTED,
        false,
    ));
}

fn item(
    section: Section,
    state: (bool, bool),
    at: (PxRect, bool),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (focused, current) = state;
    let (rect, rail) = at;
    let scale = input.scale;
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    let background = match (current, hovered) {
        (true, _) => Some(NAV_CURRENT),
        (false, true) => Some(NAV_HOVER),
        (false, false) => None,
    };
    if let Some(color) = background {
        let quad = rounded(rect, color, NAV_ITEM_RADIUS, scale);
        scene.quads.push(match focused {
            true => Quad {
                border: Some(Border {
                    color: opaque(PRIMARY),
                    width: px(LINE, scale),
                }),
                ..quad
            },
            false => quad,
        });
    }
    let (text, icon): (Rgb, Rgb) = match current || hovered {
        true => (FOREGROUND, FOREGROUND),
        false => (QUIET, ICON),
    };
    let size = px(ICON_BOX, scale);
    let icon_box = PxRect {
        x: match rail {
            true => rect.x + ((rect.w - size) / 2.0).round(),
            false => rect.x + px(NAV_ITEM_PAD, scale),
        },
        y: rect.y + ((rect.h - size) / 2.0).round(),
        w: size,
        h: size,
    };
    scene
        .text
        .push(glyph(section_icon(section), icon_box, icon, input.metrics));
    if rail {
        return;
    }
    let x = icon_box.x + size + px(ICON_GAP, scale);
    let room = rect.x + rect.w - px(NAV_ITEM_PAD, scale) - x;
    scene.text.push(ui_text(
        x,
        baseline(rect, input.ui),
        ellipsize(section.title(), room, false, measure),
        text,
        false,
    ));
}

fn section_icon(section: Section) -> char {
    match section {
        Section::Appearance => APPEARANCE_ICON,
        Section::Font => FONT_ICON,
        Section::Window => WINDOW_ICON,
        Section::Panels => PANELS_ICON,
        Section::Terminal => TERMINAL_ICON,
        Section::Prompt => PROMPT_ICON,
        Section::Keys => KEYS_ICON,
    }
}

fn group(section: Section) -> Option<&'static str> {
    match section {
        Section::Appearance | Section::Font | Section::Window | Section::Panels => None,
        Section::Terminal | Section::Prompt => Some("Terminal"),
        Section::Keys => Some("Input"),
    }
}
