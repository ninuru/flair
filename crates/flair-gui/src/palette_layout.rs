use crate::{
    chrome_bar::{baseline, ellipsize, fill, opaque, rounded, small_text, ui_text},
    chrome_layout::bar_height,
    overlay_layout::{OverlayInput, OverlayScene, Placement, Sheet, shadow, visible},
    overlay_rows::entry,
    preview_layout::{self, Strip},
    renderer::{Border, PxRect, Quad},
    settings_rows::glyph,
    t3_style::{
        DIM, EMPTY_HEIGHT, FIELD_ICON_X, FIELD_PAD_X, FIELD_PAD_Y, FIELD_ROW, FIELD_TEXT_X,
        FOOTER_HEIGHT, FOOTER_PAD_X, FOREGROUND, GROUP_LABEL, HINT_GAP, HINT_INNER, ICON, ITEM_ROW,
        KBD_HEIGHT, KBD_PAD, KBD_RADIUS, LIST_PAD, MUTED, PALETTE, PALETTE_BAR_GAP,
        PALETTE_DOCK_WIDTH, PALETTE_FOOTER, PALETTE_FOOTER_BORDER, PALETTE_KEYCAP, PALETTE_MAX_H,
        PALETTE_SHINE, PALETTE_SIDE, PALETTE_TOP, PALETTE_WIDTH, PANEL_BORDER, PANEL_RADIUS, QUIET,
        STRIP_PAD,
    },
    ui::{LINE, px},
};

struct Plan {
    field: f32,
    label: f32,
    rows: usize,
    empty: f32,
    strip: f32,
    footer: f32,
}

impl Plan {
    fn list(&self, scale: f32) -> f32 {
        let body = self.label + self.rows as f32 * px(ITEM_ROW, scale) + self.empty;
        match body > 0.0 {
            true => body + px(LIST_PAD, scale) * 2.0,
            false => 0.0,
        }
    }

    fn height(&self, scale: f32) -> f32 {
        self.field + self.list(scale) + self.strip + self.footer
    }
}

pub(crate) fn layout(
    sheet: &Sheet,
    input: &OverlayInput,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> OverlayScene {
    let scale = input.scale;
    let (width, height) = (input.px.0 as f32, input.px.1 as f32);
    let window = PxRect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: height,
    };
    let mut scene = OverlayScene::default();
    let side = px(PALETTE_SIDE, scale);
    let below_bar = bar_height(scale) + px(PALETTE_BAR_GAP, scale);
    let (panel_width, top, max_height) = match sheet.placement {
        Placement::Centred => {
            scene.quads.push(Quad {
                rect: window,
                color: DIM,
                radius: 0.0,
                border: None,
            });
            let top = (height * PALETTE_TOP).round().max(below_bar);
            (
                px(PALETTE_WIDTH, scale).min(width - side * 2.0),
                top,
                px(PALETTE_MAX_H, scale).min(height - top - side),
            )
        }
        Placement::Docked => (
            px(PALETTE_DOCK_WIDTH, scale).min(width - side * 2.0),
            below_bar,
            height - below_bar - px(PALETTE_BAR_GAP, scale),
        ),
    };
    let panel_width = panel_width.max(0.0).round();
    let plan = plan(sheet, max_height.max(0.0), scale);
    let panel = PxRect {
        x: match sheet.placement {
            Placement::Centred => ((width - panel_width) / 2.0).round(),
            Placement::Docked => width - side - panel_width,
        },
        y: top,
        w: panel_width,
        h: plan.height(scale).min(max_height.max(0.0)),
    };
    scene.hits.panel = panel;
    if panel.w <= 0.0 || panel.h <= 0.0 {
        return scene;
    }
    shadow(panel, window, scale, &mut scene);
    let line = px(LINE, scale);
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(PANEL_BORDER),
            width: line,
        }),
        ..rounded(panel, PALETTE, PANEL_RADIUS, scale)
    });
    let radius = px(PANEL_RADIUS, scale);
    scene.quads.push(fill(
        PxRect {
            x: panel.x + radius,
            y: panel.y + line,
            w: (panel.w - radius * 2.0).max(0.0),
            h: line,
        },
        PALETTE_SHINE,
    ));
    let mut y = panel.y;
    if plan.field > 0.0 {
        field_row(sheet, panel, input, &mut scene, measure);
        y += plan.field;
    }
    if plan.list(scale) > 0.0 {
        list(sheet, &plan, (panel, y), input, &mut scene, measure);
        y += plan.list(scale);
    }
    if let (true, Some(strip)) = (plan.strip > 0.0, &sheet.strip) {
        preview(strip, PxRect { y, ..panel }, input, &mut scene, measure);
    }
    if plan.footer > 0.0 {
        footer(&sheet.keys, panel, input, &mut scene, measure);
    }
    scene
}

fn plan(sheet: &Sheet, room: f32, scale: f32) -> Plan {
    let field = match sheet.field {
        Some(_) => px(FIELD_ROW, scale) + px(FIELD_PAD_Y, scale) * 2.0,
        None => 0.0,
    };
    let label = match sheet.title {
        Some(_) => px(GROUP_LABEL, scale),
        None => 0.0,
    };
    let empty = match (sheet.rows.is_empty(), sheet.empty) {
        (true, Some(_)) => px(EMPTY_HEIGHT, scale),
        (true, None) | (false, _) => 0.0,
    };
    let footer = match sheet.keys.is_empty() {
        true => 0.0,
        false => px(FOOTER_HEIGHT, scale),
    };
    let strip = match &sheet.strip {
        Some(strip) if !strip.windows.is_empty() => {
            preview_layout::strip_height(scale) + px(STRIP_PAD, scale) * 2.0 + px(LINE, scale)
        }
        Some(_) | None => 0.0,
    };
    let row = px(ITEM_ROW, scale);
    let pads = px(LIST_PAD, scale) * 2.0;
    if sheet.rows.is_empty() {
        return empty_plan((field, label, empty), (strip, footer), room, scale);
    }
    let fit = |extra: f32| {
        let free = room - field - label - empty - pads - extra;
        ((free / row).floor().max(0.0) as usize).min(sheet.rows.len())
    };
    let wanted = sheet.rows.len().min(1);
    let (strip, footer) = if fit(strip + footer) >= wanted.max(3).min(sheet.rows.len()) {
        (strip, footer)
    } else if fit(footer) >= wanted {
        (0.0, footer)
    } else {
        (0.0, 0.0)
    };
    let rows = fit(strip + footer);
    let field = match field <= room {
        true => field,
        false => 0.0,
    };
    Plan {
        field,
        label,
        rows,
        empty,
        strip,
        footer,
    }
}

fn empty_plan(heights: (f32, f32, f32), extras: (f32, f32), room: f32, scale: f32) -> Plan {
    let (field, label, wanted) = heights;
    let (strip, footer) = extras;
    let field = match field <= room {
        true => field,
        false => 0.0,
    };
    let pads = match label > 0.0 || wanted > 0.0 {
        true => px(LIST_PAD, scale) * 2.0,
        false => 0.0,
    };
    let body = (room - field - label - pads).max(0.0);
    let (empty, footer) = if body - footer >= wanted {
        (wanted, footer)
    } else if body >= wanted {
        (wanted, 0.0)
    } else if body >= px(ITEM_ROW, scale) && wanted > 0.0 {
        (body.floor(), 0.0)
    } else {
        (0.0, 0.0)
    };
    let label = match label + empty + pads <= room - field {
        true => label,
        false => 0.0,
    };
    let strip = match body - empty - footer >= strip {
        true => strip,
        false => 0.0,
    };
    Plan {
        field,
        label,
        rows: 0,
        empty,
        strip,
        footer,
    }
}

fn field_row(
    sheet: &Sheet,
    panel: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let Some(field) = &sheet.field else { return };
    let scale = input.scale;
    let rect = PxRect {
        x: panel.x + px(FIELD_PAD_X, scale),
        y: panel.y + px(FIELD_PAD_Y, scale),
        w: (panel.w - px(FIELD_PAD_X, scale) * 2.0).max(0.0),
        h: px(FIELD_ROW, scale),
    };
    let size = px(16.0, scale);
    scene.text.push(glyph(
        sheet.glyph,
        PxRect {
            x: rect.x + px(FIELD_ICON_X, scale),
            y: rect.y + ((rect.h - size) / 2.0).round(),
            w: size,
            h: size,
        },
        ICON,
        input.metrics,
    ));
    let x = rect.x + px(FIELD_TEXT_X, scale);
    let end = rect.x + rect.w - px(FIELD_PAD_X, scale);
    let (shown, total) = sheet.count;
    let count = match field.text.is_empty() {
        true => total.to_string(),
        false => format!("{shown} of {total}"),
    };
    let count = match sheet.empty {
        Some(_) => count,
        None => String::new(),
    };
    let count_width = measure(&count, false) * input.ui.small;
    if !count.is_empty() {
        scene.text.push(small_text(
            (end - count_width).round(),
            rect,
            count,
            QUIET,
            input.ui,
        ));
    }
    let room = (end - count_width - px(FIELD_PAD_X, scale) - x).max(0.0);
    let base = baseline(rect, input.ui);
    let caret = match field.text.is_empty() {
        true => {
            scene.text.push(ui_text(
                x,
                base,
                ellipsize(field.placeholder, room, false, measure),
                MUTED,
                false,
            ));
            0.0
        }
        false => {
            let (text, caret) = visible(&field.text, field.cursor, room, measure);
            scene.text.push(ui_text(x, base, text, FOREGROUND, false));
            caret
        }
    };
    scene.quads.push(fill(
        PxRect {
            x: (x + caret).round(),
            y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
            w: px(LINE, scale),
            h: input.ui.height.round(),
        },
        FOREGROUND,
    ));
}

fn list(
    sheet: &Sheet,
    plan: &Plan,
    at: (PxRect, f32),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (panel, top) = at;
    let scale = input.scale;
    let pad = px(LIST_PAD, scale);
    let x = panel.x + pad;
    let width = (panel.w - pad * 2.0).max(0.0);
    let mut y = top + pad;
    if let Some(title) = sheet.title {
        let rect = PxRect {
            x,
            y,
            w: width,
            h: plan.label,
        };
        scene.text.push(small_text(
            x + px(FIELD_PAD_X, scale),
            rect,
            ellipsize(title, width / input.ui.small, false, measure),
            MUTED,
            input.ui,
        ));
        y += plan.label;
    }
    if plan.empty > 0.0 {
        let text = sheet.empty.unwrap_or_default();
        let shown = ellipsize(text, width, false, measure);
        let rect = PxRect {
            x,
            y,
            w: width,
            h: plan.empty,
        };
        scene.text.push(ui_text(
            (x + (width - measure(&shown, false)) / 2.0).round(),
            baseline(rect, input.ui),
            shown,
            MUTED,
            false,
        ));
        return;
    }
    let capacity = plan.rows;
    let first = sheet.selected.saturating_sub(capacity.saturating_sub(1));
    let gutters: Vec<u8> = sheet
        .rows
        .iter()
        .filter(|row| row.icon.is_some() || row.monogram)
        .map(|row| row.depth)
        .collect();
    let row_height = px(ITEM_ROW, scale);
    for (offset, row) in sheet.rows.iter().skip(first).take(capacity).enumerate() {
        let index = first + offset;
        let rect = PxRect {
            x,
            y: y + offset as f32 * row_height,
            w: width,
            h: row_height,
        };
        entry(
            row,
            index == sheet.selected,
            gutters.contains(&row.depth),
            rect,
            input,
            scene,
            measure,
        );
        scene.hits.rows.push((index, rect));
    }
}

fn preview(
    strip: &Strip,
    area: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let line = px(LINE, scale);
    scene.quads.push(fill(
        PxRect {
            x: area.x + line,
            y: area.y,
            w: (area.w - line * 2.0).max(0.0),
            h: line,
        },
        PALETTE_FOOTER_BORDER,
    ));
    let pad = px(STRIP_PAD, scale);
    preview_layout::strip(
        strip,
        PxRect {
            x: area.x + px(FOOTER_PAD_X, scale),
            y: area.y + line + pad,
            w: (area.w - px(FOOTER_PAD_X, scale) * 2.0).max(0.0),
            h: preview_layout::strip_height(scale),
        },
        input,
        scene,
        measure,
    );
}

fn footer(
    keys: &[(&'static str, String)],
    panel: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let line = px(LINE, scale);
    let height = px(FOOTER_HEIGHT, scale);
    let rect = PxRect {
        x: panel.x + line,
        y: panel.y + panel.h - line - height,
        w: (panel.w - line * 2.0).max(0.0),
        h: height,
    };
    let radius = px(PANEL_RADIUS, scale) - line;
    scene
        .quads
        .push(rounded(rect, PALETTE_FOOTER, PANEL_RADIUS - LINE, scale));
    scene.quads.push(fill(
        PxRect {
            h: (rect.h - radius).max(0.0),
            ..rect
        },
        PALETTE_FOOTER,
    ));
    scene
        .quads
        .push(fill(PxRect { h: line, ..rect }, PALETTE_FOOTER_BORDER));
    let cap_height = px(KBD_HEIGHT, scale);
    let mut x = rect.x + px(FOOTER_PAD_X, scale);
    let end = rect.x + rect.w - px(FOOTER_PAD_X, scale);
    for (key, verb) in keys {
        let key_width = measure(key, false) * input.ui.small;
        let cap = PxRect {
            x: x.round(),
            y: (rect.y + (rect.h - cap_height) / 2.0).round(),
            w: (key_width + px(KBD_PAD, scale) * 2.0)
                .max(cap_height)
                .round(),
            h: cap_height,
        };
        let verb_x = cap.x + cap.w + px(HINT_INNER, scale);
        let verb_width = measure(verb, false);
        if verb_x + verb_width > end {
            break;
        }
        scene
            .quads
            .push(rounded(cap, PALETTE_KEYCAP, KBD_RADIUS, scale));
        scene.text.push(small_text(
            (cap.x + (cap.w - key_width) / 2.0).round(),
            cap,
            (*key).to_owned(),
            FOREGROUND,
            input.ui,
        ));
        scene.text.push(ui_text(
            verb_x.round(),
            baseline(rect, input.ui),
            verb.clone(),
            MUTED,
            false,
        ));
        x = verb_x + verb_width + px(HINT_GAP, scale);
    }
}
