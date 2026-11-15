use flair_core::theme::{Rgb, Rgba};

use crate::{
    CellMetrics,
    chrome_bar::{baseline, ellipsize, icon_text, opaque, rounded, small_text, ui_text},
    isles::icon_width,
    overlay_layout::{Field, OverlayInput, OverlayScene, inside, visible},
    renderer::{Border, PxRect, Quad, TextRun},
    settings_layout::{Control, Entry, SettingsHit},
    settings_style::{
        CHEVRON, CHEVRON_ICON, CHORD, CHORD_GAP, CHORD_PAD, CONTROL_GAP, DESCRIPTION_GAP,
        DESCRIPTION_LINE, ERROR, ERROR_LINE, FOCUS_INSET, FOCUS_RADIUS, FOREGROUND, GHOST_HOVER,
        ICON_BOX, INPUT, INPUT_BORDER, KBD_HEIGHT, KBD_RADIUS, MINUS_ICON, MUTED, PLUS_ICON,
        PRIMARY, QUIET, RING, ROW_FOCUS, ROW_HOVER, ROW_PAD_X, ROW_PAD_Y, ROW_SHORT, ROW_TALL,
        SELECT_HEIGHT, SELECT_PAD, SELECT_RADIUS, SELECT_WIDTH, STEP_ZONE, SWATCH, SWATCH_GAP,
        SWATCH_RADIUS, SWITCH_HEIGHT, SWITCH_OFF, SWITCH_PAD, SWITCH_THUMB_ON, SWITCH_WIDTH,
        TEXT_WIDTH, THUMB, TITLE_LINE, UNDO_GAP, UNDO_ICON, UNDO_RADIUS, UNDO_SIZE,
    },
    ui::{LINE, px},
};

pub(crate) fn entry_height(entry: &Entry, scale: f32) -> f32 {
    match entry {
        Entry::Setting { error: Some(_), .. } => px(ROW_TALL, scale) + px(ERROR_LINE, scale),
        Entry::Setting { error: None, .. } => px(ROW_TALL, scale),
        Entry::Binding { .. } => px(ROW_SHORT, scale),
    }
}

pub(crate) fn glyph(icon: char, area: PxRect, color: Rgb, metrics: CellMetrics) -> TextRun {
    icon_text(
        area.x + (area.w - icon_width(icon, metrics)) / 2.0,
        area.y + area.h / 2.0,
        icon,
        color,
        metrics,
    )
}

pub(crate) fn entry_row(
    entry: &Entry,
    item: usize,
    rect: PxRect,
    state: (bool, bool),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (focused, open) = state;
    highlight(rect, focused, input, scene);
    let scale = input.scale;
    let right = rect.x + rect.w - px(ROW_PAD_X, scale);
    match entry {
        Entry::Setting {
            title,
            description,
            modified,
            control,
            error,
            pending,
        } => {
            let main = PxRect {
                h: px(ROW_TALL, scale),
                ..rect
            };
            let at = (right, main);
            let left = match control {
                Control::Toggle(on) => switch(*on, item, at, input, scene),
                Control::Number(value) => number(value, item, at, input, scene, measure),
                Control::Choice(value) => select(
                    value,
                    (SettingsHit::Cycle(item), open),
                    at,
                    input,
                    scene,
                    measure,
                ),
                Control::Text(value) => text_field(value, item, at, input, scene, measure),
                Control::Color(value) => colour(value, item, at, input, scene, measure),
                Control::Editor(field) => editor(field, at, input, scene, measure),
            };
            let end = left - px(CONTROL_GAP, scale);
            let undo = (*modified || *pending).then_some(item);
            describe(
                (title, description),
                undo,
                (end, main),
                input,
                scene,
                measure,
            );
            if let Some(error) = error {
                error_line(error, rect, input, scene, measure);
            }
        }
        Entry::Binding {
            chord,
            command,
            unbound,
            table,
            disabled,
        } => {
            let cap = chord_cap(chord, (right, rect), input, scene, measure);
            let label_width = measure(table, false) * input.ui.small;
            let label_x = (cap - px(CHORD_GAP, scale) - label_width).round();
            scene.text.push(small_text(
                label_x,
                rect,
                (*table).to_owned(),
                QUIET,
                input.ui,
            ));
            let x = rect.x + px(ROW_PAD_X, scale);
            let color = match *unbound || *disabled {
                true => MUTED,
                false => FOREGROUND,
            };
            scene.text.push(ui_text(
                x,
                baseline(rect, input.ui),
                ellipsize(
                    command,
                    label_x - px(CONTROL_GAP, scale) - x,
                    false,
                    measure,
                ),
                color,
                false,
            ));
        }
    }
}

fn highlight(rect: PxRect, focused: bool, input: &OverlayInput, scene: &mut OverlayScene) {
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    let color = match (focused, hovered) {
        (true, _) => ROW_FOCUS,
        (false, true) => ROW_HOVER,
        (false, false) => return,
    };
    let inset = px(FOCUS_INSET, input.scale);
    let area = PxRect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - inset * 2.0).max(0.0),
        h: (rect.h - inset * 2.0).max(0.0),
    };
    scene
        .quads
        .push(rounded(area, color, FOCUS_RADIUS, input.scale));
}

fn describe(
    text: (&str, &str),
    undo: Option<usize>,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (title, description) = text;
    let (end, row) = at;
    let scale = input.scale;
    let x = row.x + px(ROW_PAD_X, scale);
    let width = (end - x).max(0.0);
    let title_box = PxRect {
        x,
        y: row.y + px(ROW_PAD_Y, scale),
        w: width,
        h: px(TITLE_LINE, scale),
    };
    let undo_room = match undo {
        Some(_) => px(UNDO_GAP, scale) + px(UNDO_SIZE, scale),
        None => 0.0,
    };
    let shown = ellipsize(title, width - undo_room, false, measure);
    let title_end = x + measure(&shown, false);
    scene.text.push(ui_text(
        x,
        baseline(title_box, input.ui),
        shown,
        FOREGROUND,
        false,
    ));
    if let Some(item) = undo
        && width >= undo_room
    {
        let size = px(UNDO_SIZE, scale);
        let button = PxRect {
            x: (title_end + px(UNDO_GAP, scale)).round(),
            y: (title_box.y + (title_box.h - size) / 2.0).round(),
            w: size,
            h: size,
        };
        if input.hover.is_some_and(|point| inside(button, point)) {
            scene
                .quads
                .push(rounded(button, GHOST_HOVER, UNDO_RADIUS, scale));
        }
        scene
            .text
            .push(glyph(UNDO_ICON, button, MUTED, input.metrics));
        scene.hits.controls.push((SettingsHit::Reset(item), button));
    }
    let description_box = PxRect {
        y: title_box.y + title_box.h + px(DESCRIPTION_GAP, scale),
        h: px(DESCRIPTION_LINE, scale),
        ..title_box
    };
    scene.text.push(small_text(
        x,
        description_box,
        ellipsize(description, width / input.ui.small, false, measure),
        MUTED,
        input.ui,
    ));
}

fn error_line(
    error: &str,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let pad = px(ROW_PAD_X, scale);
    let line = PxRect {
        x: rect.x + pad,
        y: rect.y
            + px(ROW_PAD_Y, scale)
            + px(TITLE_LINE, scale)
            + px(DESCRIPTION_GAP, scale)
            + px(DESCRIPTION_LINE, scale),
        w: (rect.w - pad * 2.0).max(0.0),
        h: px(ERROR_LINE, scale),
    };
    scene.text.push(small_text(
        line.x,
        line,
        ellipsize(error, line.w / input.ui.small, false, measure),
        ERROR,
        input.ui,
    ));
}

fn centred(at: (f32, PxRect), width: f32, height: f32) -> PxRect {
    let (right, row) = at;
    let width = width.min(((right - row.x) / 2.0).floor()).max(0.0);
    PxRect {
        x: (right - width).round(),
        y: (row.y + (row.h - height) / 2.0).round(),
        w: width,
        h: height,
    }
}

fn trigger(rect: PxRect, ring: bool, input: &OverlayInput, scene: &mut OverlayScene) {
    let border = match ring {
        true => PRIMARY,
        false => INPUT_BORDER,
    };
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(border),
            width: px(LINE, input.scale),
        }),
        ..rounded(rect, INPUT, SELECT_RADIUS, input.scale)
    });
}

fn switch(
    on: bool,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
) -> f32 {
    let scale = input.scale;
    let track = centred(at, px(SWITCH_WIDTH, scale), px(SWITCH_HEIGHT, scale));
    let (color, thumb_color) = match on {
        true => (PRIMARY, SWITCH_THUMB_ON),
        false => (SWITCH_OFF, MUTED),
    };
    scene
        .quads
        .push(rounded(track, color, SWITCH_HEIGHT / 2.0, scale));
    let thumb = px(THUMB, scale);
    let pad = px(SWITCH_PAD, scale);
    let x = match on {
        true => track.x + track.w - thumb - pad,
        false => track.x + pad,
    };
    scene.quads.push(rounded(
        PxRect {
            x,
            y: track.y + pad,
            w: thumb,
            h: thumb,
        },
        thumb_color,
        THUMB / 2.0,
        scale,
    ));
    let ring = px(RING, scale);
    let area = PxRect {
        x: track.x - ring,
        y: track.y - ring,
        w: track.w + ring * 2.0,
        h: track.h + ring * 2.0,
    };
    scene.hits.controls.push((SettingsHit::Toggle(item), area));
    track.x
}

fn value_text(
    value: &str,
    x: f32,
    room: f32,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    scene.text.push(ui_text(
        x.round(),
        baseline(rect, input.ui),
        ellipsize(value, room, false, measure),
        FOREGROUND,
        false,
    ));
}

fn select(
    value: &str,
    action: (SettingsHit, bool),
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (hit, open) = action;
    let scale = input.scale;
    let rect = centred(at, px(SELECT_WIDTH, scale), px(SELECT_HEIGHT, scale));
    trigger(rect, open, input, scene);
    let pad = px(SELECT_PAD, scale);
    let size = px(ICON_BOX, scale);
    let chevron = PxRect {
        x: rect.x + rect.w - pad - size,
        w: size,
        ..rect
    };
    scene
        .text
        .push(glyph(CHEVRON_ICON, chevron, CHEVRON, input.metrics));
    let x = rect.x + pad;
    value_text(value, x, chevron.x - x - pad, rect, input, scene, measure);
    scene.hits.controls.push((hit, rect));
    rect.x
}

fn text_field(
    value: &str,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let rect = centred(at, px(TEXT_WIDTH, scale), px(SELECT_HEIGHT, scale));
    trigger(rect, false, input, scene);
    let pad = px(SELECT_PAD, scale);
    value_text(
        value,
        rect.x + pad,
        rect.w - pad * 2.0,
        rect,
        input,
        scene,
        measure,
    );
    scene.hits.controls.push((SettingsHit::Edit(item), rect));
    rect.x
}

fn number(
    value: &str,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let rect = centred(at, px(SELECT_WIDTH, scale), px(SELECT_HEIGHT, scale));
    trigger(rect, false, input, scene);
    let zone = px(STEP_ZONE, scale);
    let pad = px(SELECT_PAD, scale);
    let plus = PxRect {
        x: rect.x + rect.w - px(LINE, scale) - zone,
        w: zone,
        ..rect
    };
    let minus = PxRect {
        x: plus.x - zone,
        ..plus
    };
    for (icon, area, hit) in [
        (MINUS_ICON, minus, SettingsHit::Decrease(item)),
        (PLUS_ICON, plus, SettingsHit::Increase(item)),
    ] {
        if input.hover.is_some_and(|point| inside(area, point)) {
            scene.quads.push(rounded(
                PxRect {
                    y: area.y + px(LINE, scale) * 3.0,
                    h: area.h - px(LINE, scale) * 6.0,
                    ..area
                },
                GHOST_HOVER,
                KBD_RADIUS,
                scale,
            ));
        }
        scene.text.push(glyph(icon, area, MUTED, input.metrics));
        scene.hits.controls.push((hit, area));
    }
    let x = rect.x + pad;
    value_text(value, x, minus.x - x - pad, rect, input, scene, measure);
    rect.x
}

fn colour(
    value: &str,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let Ok(color) = Rgba::from_hex(value) else {
        return text_field(value, item, at, input, scene, measure);
    };
    let scale = input.scale;
    let rect = centred(at, px(SELECT_WIDTH, scale), px(SELECT_HEIGHT, scale));
    trigger(rect, false, input, scene);
    let pad = px(SELECT_PAD, scale);
    let size = px(SWATCH, scale);
    scene.quads.push(Quad {
        rect: PxRect {
            x: rect.x + pad,
            y: (rect.y + (rect.h - size) / 2.0).round(),
            w: size,
            h: size,
        },
        color,
        radius: px(SWATCH_RADIUS, scale),
        border: Some(Border {
            color: opaque(INPUT_BORDER),
            width: px(LINE, scale),
        }),
    });
    let x = rect.x + pad + size + px(SWATCH_GAP, scale);
    value_text(
        value,
        x,
        rect.x + rect.w - pad - x,
        rect,
        input,
        scene,
        measure,
    );
    scene.hits.controls.push((SettingsHit::Edit(item), rect));
    rect.x
}

fn editor(
    field: &Field,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let rect = centred(at, px(TEXT_WIDTH, scale), px(SELECT_HEIGHT, scale));
    trigger(rect, true, input, scene);
    let pad = px(SELECT_PAD, scale);
    let x = rect.x + pad;
    let room = (rect.w - pad * 2.0).max(0.0);
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
    scene.quads.push(Quad {
        rect: PxRect {
            x: (x + caret).round(),
            y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
            w: px(LINE, scale),
            h: input.ui.height.round(),
        },
        color: opaque(FOREGROUND),
        radius: 0.0,
        border: None,
    });
    rect.x
}

fn chord_cap(
    chord: &str,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (right, row) = at;
    let scale = input.scale;
    let height = px(KBD_HEIGHT, scale);
    let text_width = measure(chord, false) * input.ui.small;
    let width = (text_width + px(CHORD_PAD, scale) * 2.0)
        .max(height)
        .round();
    let rect = PxRect {
        x: (right - width).round(),
        y: (row.y + (row.h - height) / 2.0).round(),
        w: width,
        h: height,
    };
    scene.quads.push(rounded(rect, CHORD, KBD_RADIUS, scale));
    scene.text.push(small_text(
        (rect.x + (width - text_width) / 2.0).round(),
        rect,
        chord.to_owned(),
        FOREGROUND,
        input.ui,
    ));
    rect.x
}
