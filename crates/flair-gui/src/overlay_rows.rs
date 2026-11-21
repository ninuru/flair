use crate::{
    chrome_bar::{baseline, fill, rounded, small_text, ui_text},
    overlay_layout::{OverlayInput, OverlayScene, Row, inside},
    renderer::PxRect,
    settings_rows::glyph,
    t3_style::{
        FOREGROUND, ICON_BOX, ITEM_GAP, ITEM_INDENT, ITEM_PAD_X, ITEM_RADIUS, MONOGRAM, MUTED,
        NAME_MAX, PALETTE_HIGHLIGHT, QUIET,
    },
    ui::{ELLIPSIS, SELECT_BAR, SELECT_INSET, px},
};

pub(crate) fn entry(
    row: &Row,
    selected: bool,
    columns: (bool, f32),
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (gutter, column) = columns;
    let scale = input.scale;
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    if selected || hovered {
        scene
            .quads
            .push(rounded(rect, PALETTE_HIGHLIGHT, ITEM_RADIUS, scale));
    }
    if row.current {
        let inset = px(SELECT_INSET, scale);
        scene.quads.push(fill(
            PxRect {
                x: rect.x,
                y: rect.y + inset,
                w: px(SELECT_BAR, scale),
                h: (rect.h - inset * 2.0).max(0.0),
            },
            input.theme.accent,
        ));
    }
    let pad = px(ITEM_PAD_X, scale);
    let gap = px(ITEM_GAP, scale);
    let mut x = rect.x + pad + f32::from(row.depth) * px(ITEM_INDENT, scale);
    let line = baseline(rect, input.ui);
    if gutter {
        let size = px(ICON_BOX, scale);
        let lead = PxRect {
            x: x.round(),
            y: (rect.y + (rect.h - size) / 2.0).round(),
            w: size,
            h: size,
        };
        match (row.icon, row.monogram) {
            (Some(icon), _) => scene.text.push(glyph(icon, lead, MUTED, input.metrics)),
            (None, true) => monogram(&row.name, lead, input, scene, measure),
            (None, false) => {}
        }
        x += size + gap;
    }
    let mut end = rect.x + rect.w - pad;
    match row.number {
        Some(number) => {
            let text = number.to_string();
            let width = measure(&text, false) * input.ui.small;
            end -= width;
            scene
                .text
                .push(small_text(end.round(), rect, text, QUIET, input.ui));
            end -= gap;
        }
        None if column > 0.0 => end -= column + gap,
        None => {}
    }
    let label = row
        .label
        .map(|label| (label, measure(label, false) * input.ui.small));
    let reserved = label.map_or(0.0, |(_, width)| width + gap);
    let room = (end - x - reserved).max(0.0);
    let name_room = match row.detail {
        Some(_) => room.min(px(NAME_MAX, scale)),
        None => room,
    };
    let color = match row.muted {
        true => MUTED,
        false => FOREGROUND,
    };
    let name = segments(&marked(&row.name, &row.name_matches), name_room, measure);
    for (text, matched) in name {
        let width = measure(&text, matched);
        scene
            .text
            .push(ui_text(x.round(), line, text, color, matched));
        x += width;
    }
    let detail_room = (end - x - gap - reserved).max(0.0);
    let start = match &row.detail {
        Some(detail) => detail_runs(
            &marked(detail, &row.detail_matches),
            (end, detail_room, rect),
            input,
            scene,
            measure,
        ),
        None => end,
    };
    if let Some((label, width)) = label {
        let at = match start < end {
            true => start - gap - width,
            false => end - width,
        };
        scene.text.push(small_text(
            at.round(),
            rect,
            label.to_owned(),
            QUIET,
            input.ui,
        ));
    }
}

fn monogram(
    name: &str,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let Some(letter) = name.chars().find(|c| c.is_alphanumeric()) else {
        return;
    };
    let letter: String = letter.to_uppercase().collect();
    scene
        .quads
        .push(rounded(rect, MONOGRAM, ITEM_RADIUS, input.scale));
    let width = measure(&letter, false) * input.ui.small;
    scene.text.push(small_text(
        (rect.x + (rect.w - width) / 2.0).round(),
        rect,
        letter,
        FOREGROUND,
        input.ui,
    ));
}

pub(crate) fn marked(text: &str, matches: &[usize]) -> Vec<(char, bool)> {
    text.chars()
        .enumerate()
        .filter(|(_, c)| !c.is_control())
        .map(|(index, c)| (c, matches.binary_search(&index).is_ok()))
        .collect()
}

pub(crate) fn segments(
    chars: &[(char, bool)],
    max: f32,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> Vec<(String, bool)> {
    let widths: Vec<f32> = chars
        .iter()
        .map(|(c, matched)| measure(c.encode_utf8(&mut [0; 4]), *matched))
        .collect();
    let kept: Vec<(char, bool)> = match widths.iter().sum::<f32>() <= max {
        true => chars.to_vec(),
        false => {
            let ellipsis = measure(ELLIPSIS, false);
            if ellipsis > max {
                return Vec::new();
            }
            let mut used = ellipsis;
            let count = widths
                .iter()
                .take_while(|width| {
                    used += *width;
                    used <= max
                })
                .count();
            chars[..count]
                .iter()
                .copied()
                .chain(ELLIPSIS.chars().map(|c| (c, false)))
                .collect()
        }
    };
    group(&kept)
}

fn group(chars: &[(char, bool)]) -> Vec<(String, bool)> {
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (c, matched) in chars {
        match runs.last_mut() {
            Some((text, last)) if last == matched => text.push(*c),
            Some(_) | None => runs.push((c.to_string(), *matched)),
        }
    }
    runs
}

fn detail_runs(
    chars: &[(char, bool)],
    area: (f32, f32, PxRect),
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (end, room, rect) = area;
    let small = input.ui.small;
    let mut width = |text: &str| measure(text, false) * small;
    let widths: Vec<f32> = chars
        .iter()
        .map(|(c, _)| width(c.encode_utf8(&mut [0; 4])))
        .collect();
    let kept: Vec<(char, bool)> = match widths.iter().sum::<f32>() <= room {
        true => chars.to_vec(),
        false => {
            let mut used = width(ELLIPSIS);
            if used > room {
                return end;
            }
            let count = widths
                .iter()
                .rev()
                .take_while(|advance| {
                    used += *advance;
                    used <= room
                })
                .count();
            ELLIPSIS
                .chars()
                .map(|c| (c, false))
                .chain(chars[chars.len() - count..].iter().copied())
                .collect()
        }
    };
    let runs = group(&kept);
    let total: f32 = runs.iter().map(|(text, _)| width(text)).sum();
    let start = (end - total).round();
    let mut x = start;
    for (text, matched) in runs {
        let advance = width(&text);
        let color = match matched {
            true => FOREGROUND,
            false => QUIET,
        };
        scene
            .text
            .push(small_text(x.round(), rect, text, color, input.ui));
        x += advance;
    }
    start
}
