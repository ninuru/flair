use crate::{
    chrome_bar::{baseline, ellipsize, opaque, rounded, small_text, ui_text},
    overlay_layout::{Field, OverlayInput, OverlayScene, cap, inside, search},
    renderer::{Border, PxRect, Quad},
    settings_layout::{Control, Entry, SettingsHit},
    ui::{
        CHIP_MIN, CHOICE_MARK, CHORD_COLUMN, CONTROL_GAP, CONTROL_MAX, ENTRY_HEIGHT, ENTRY_PAD,
        ENTRY_TOP, ERROR_HEIGHT, HIGHLIGHT_INSET, ITEM_GAP, ITEM_HEIGHT, ITEM_PAD, KNOB,
        KNOB_INSET, LINE, LINE_BOX, ROW_HEIGHT, ROW_RADIUS, STEP_DOWN, STEP_UP, STEP_ZONE,
        TOGGLE_HEIGHT, TOGGLE_RADIUS, TOGGLE_WIDTH, px,
    },
};

pub(crate) fn entry_height(entry: &Entry, scale: f32) -> f32 {
    match entry {
        Entry::Setting { error: Some(_), .. } => px(ENTRY_HEIGHT, scale) + px(ERROR_HEIGHT, scale),
        Entry::Setting { error: None, .. } => px(ENTRY_HEIGHT, scale),
        Entry::Binding { .. } => px(ROW_HEIGHT, scale),
    }
}

pub(crate) fn entry_row(
    entry: &Entry,
    item: usize,
    rect: PxRect,
    focused: bool,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    highlight(rect, focused, input, scene);
    let scale = input.scale;
    let theme = input.theme;
    let pad = px(ENTRY_PAD, scale);
    let gap = px(CONTROL_GAP, scale);
    let right = rect.x + rect.w - pad;
    match entry {
        Entry::Setting {
            title,
            description,
            source,
            control,
            error,
        } => {
            let column = PxRect {
                h: px(ENTRY_HEIGHT, scale),
                ..rect
            };
            let at = (right, column);
            let left = match control {
                Control::Toggle(on) => toggle(*on, item, at, input, scene),
                Control::Number(value) => stepper(value, item, at, input, scene, measure),
                Control::Choice(value) => chip(
                    value,
                    Some(CHOICE_MARK),
                    SettingsHit::Cycle(item),
                    at,
                    input,
                    scene,
                    measure,
                ),
                Control::Text(value) => chip(
                    value,
                    None,
                    SettingsHit::Edit(item),
                    at,
                    input,
                    scene,
                    measure,
                ),
                Control::Editor(field) => editor(field, at, input, scene, measure),
            };
            let label_left = label(source, (left - gap, column), input, scene, measure);
            describe(
                (title, description),
                (label_left - gap, column),
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
            source,
        } => {
            cap(chord, (rect.x + pad, rect), input, scene, measure);
            let source_left = label(source, (right, rect), input, scene, measure);
            let table_left = label(table, (source_left - gap, rect), input, scene, measure);
            let x = rect.x + pad + px(CHORD_COLUMN, scale);
            let color = match unbound {
                true => theme.surfaces().muted,
                false => theme.foreground,
            };
            scene.text.push(ui_text(
                x,
                baseline(rect, input.ui),
                ellipsize(command, table_left - gap - x, false, measure),
                color,
                false,
            ));
        }
    }
}

fn highlight(rect: PxRect, focused: bool, input: &OverlayInput, scene: &mut OverlayScene) {
    let surfaces = input.theme.surfaces();
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    let color = match (focused, hovered) {
        (true, _) => surfaces.surface3,
        (false, true) => surfaces.surface2,
        (false, false) => return,
    };
    let inset = px(HIGHLIGHT_INSET, input.scale);
    let area = PxRect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - inset * 2.0).max(0.0),
        h: (rect.h - inset * 2.0).max(0.0),
    };
    scene
        .quads
        .push(rounded(area, color, ROW_RADIUS, input.scale));
}

fn label(
    text: &str,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (right, row) = at;
    let x = (right - measure(text, false) * input.ui.small).round();
    scene.text.push(small_text(
        x,
        row,
        text.to_owned(),
        input.theme.surfaces().muted,
        input.ui,
    ));
    x
}

fn describe(
    text: (&str, &str),
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (title, description) = text;
    let (end, row) = at;
    let scale = input.scale;
    let x = row.x + px(ENTRY_PAD, scale);
    let width = (end - x).max(0.0);
    let title_box = PxRect {
        x,
        y: row.y + px(ENTRY_TOP, scale),
        w: width,
        h: px(LINE_BOX, scale),
    };
    scene.text.push(ui_text(
        x,
        baseline(title_box, input.ui),
        ellipsize(title, width, false, measure),
        input.theme.foreground,
        false,
    ));
    let description_box = PxRect {
        y: title_box.y + title_box.h,
        ..title_box
    };
    scene.text.push(small_text(
        x,
        description_box,
        ellipsize(description, width / input.ui.small, false, measure),
        input.theme.surfaces().muted,
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
    let pad = px(ENTRY_PAD, scale);
    let line = PxRect {
        x: rect.x + pad,
        y: rect.y + px(ENTRY_HEIGHT, scale),
        w: (rect.w - pad * 2.0).max(0.0),
        h: px(ERROR_HEIGHT, scale),
    };
    scene.text.push(small_text(
        line.x,
        line,
        ellipsize(error, line.w / input.ui.small, false, measure),
        input.theme.surfaces().attention,
        input.ui,
    ));
}

fn room(at: (f32, PxRect), scale: f32) -> f32 {
    let (right, row) = at;
    (right - row.x - px(ENTRY_PAD, scale))
        .min(px(CONTROL_MAX, scale))
        .max(0.0)
}

fn centred(at: (f32, PxRect), width: f32, height: f32) -> PxRect {
    let (right, row) = at;
    PxRect {
        x: (right - width).round(),
        y: (row.y + (row.h - height) / 2.0).round(),
        w: width,
        h: height,
    }
}

fn shell(rect: PxRect, input: &OverlayInput, scene: &mut OverlayScene) {
    let surfaces = input.theme.surfaces();
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline_strong),
            width: px(LINE, input.scale),
        }),
        ..rounded(rect, surfaces.surface3, ROW_RADIUS, input.scale)
    });
}

fn toggle(
    on: bool,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
) -> f32 {
    let scale = input.scale;
    let theme = input.theme;
    let width = px(TOGGLE_WIDTH, scale);
    let track = centred(at, width, px(TOGGLE_HEIGHT, scale));
    let color = match on {
        true => theme.accent,
        false => theme.surfaces().surface3,
    };
    scene
        .quads
        .push(rounded(track, color, TOGGLE_RADIUS, scale));
    let knob = px(KNOB, scale);
    let inset = px(KNOB_INSET, scale);
    let x = match on {
        true => track.x + width - knob - inset,
        false => track.x + inset,
    };
    let rect = PxRect {
        x,
        y: track.y + inset,
        w: knob,
        h: knob,
    };
    scene
        .quads
        .push(rounded(rect, theme.foreground, KNOB / 2.0, scale));
    scene.hits.controls.push((SettingsHit::Toggle(item), track));
    track.x
}

fn stepper(
    value: &str,
    item: usize,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let zone = px(STEP_ZONE, scale);
    let side = zone + px(ITEM_GAP, scale);
    let value = ellipsize(value, room(at, scale) - side * 2.0, false, measure);
    let value_width = measure(&value, false);
    let width = (value_width + side * 2.0).max(px(CHIP_MIN, scale)).round();
    let rect = centred(at, width, px(ITEM_HEIGHT, scale));
    shell(rect, input, scene);
    let muted = input.theme.surfaces().muted;
    for (glyph, x, hit) in [
        (STEP_DOWN, rect.x, SettingsHit::Decrease(item)),
        (STEP_UP, rect.x + width - zone, SettingsHit::Increase(item)),
    ] {
        let area = PxRect { x, w: zone, ..rect };
        let glyph_width = measure(glyph, false) * input.ui.small;
        scene.text.push(small_text(
            (x + (zone - glyph_width) / 2.0).round(),
            area,
            glyph.to_owned(),
            muted,
            input.ui,
        ));
        scene.hits.controls.push((hit, area));
    }
    scene.text.push(ui_text(
        (rect.x + (width - value_width) / 2.0).round(),
        baseline(rect, input.ui),
        value,
        input.theme.foreground,
        false,
    ));
    rect.x
}

fn chip(
    value: &str,
    mark: Option<&str>,
    hit: SettingsHit,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let pad = px(ITEM_PAD, scale);
    let gap = px(ITEM_GAP, scale);
    let mark_width = mark.map_or(0.0, |mark| measure(mark, false) * input.ui.small + gap);
    let value = ellipsize(
        value,
        room(at, scale) - pad * 2.0 - mark_width,
        false,
        measure,
    );
    let value_width = measure(&value, false);
    let width = (pad * 2.0 + value_width + mark_width).round();
    let rect = centred(at, width, px(ITEM_HEIGHT, scale));
    shell(rect, input, scene);
    scene.text.push(ui_text(
        rect.x + pad,
        baseline(rect, input.ui),
        value,
        input.theme.foreground,
        false,
    ));
    if let Some(mark) = mark {
        scene.text.push(small_text(
            (rect.x + pad + value_width + gap).round(),
            rect,
            mark.to_owned(),
            input.theme.surfaces().muted,
            input.ui,
        ));
    }
    scene.hits.controls.push((hit, rect));
    rect.x
}

fn editor(
    field: &Field,
    at: (f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let rect = centred(at, room(at, input.scale), px(ITEM_HEIGHT, input.scale));
    search(field, rect, input, scene, measure);
    rect.x
}
