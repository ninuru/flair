use flair_core::theme::Rgb;

use crate::{
    chrome_bar::{baseline, rounded, small_text, ui_text},
    overlay_layout::{OverlayInput, OverlayScene, Row, inside},
    renderer::{PxRect, TextFace, TextRun},
    settings_rows::glyph,
    t3_style::{
        FOREGROUND, ICON_BOX, ITEM_GAP, ITEM_INDENT, ITEM_PAD_X, ITEM_RADIUS, MONOGRAM, MUTED,
        NAME_MAX, PALETTE_CURRENT, PALETTE_HIGHLIGHT, QUIET,
    },
    ui::{ELLIPSIS, px},
};

pub(crate) fn entry(
    row: &Row,
    selected: bool,
    gutter: bool,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    let background = match (selected || hovered, row.current) {
        (true, _) => Some(PALETTE_HIGHLIGHT),
        (false, true) => Some(PALETTE_CURRENT),
        (false, false) => None,
    };
    if let Some(color) = background {
        scene.quads.push(rounded(rect, color, ITEM_RADIUS, scale));
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
    if let Some(number) = row.number {
        let text = number.to_string();
        let width = measure(&text, false) * input.ui.small;
        end -= width;
        scene.text.push(small(end.round(), line, text, QUIET));
        end -= gap;
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
    let name = segments(&marked(&row.name, &row.name_matches), name_room, measure);
    for (text, matched) in name {
        let width = measure(&text, matched);
        scene
            .text
            .push(ui_text(x.round(), line, text, FOREGROUND, matched));
        x += width;
    }
    let detail_room = (end - x - gap - reserved).max(0.0);
    let start = match &row.detail {
        Some(detail) => detail_runs(
            &marked(detail, &row.detail_matches),
            (end, detail_room, line),
            input,
            scene,
        ),
        None => end,
    };
    if let Some((label, width)) = label {
        let at = match start < end {
            true => start - gap - width,
            false => end - width,
        };
        scene
            .text
            .push(small(at.round(), line, label.to_owned(), QUIET));
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

fn small(x: f32, baseline: f32, text: String, color: Rgb) -> TextRun {
    TextRun {
        x,
        baseline,
        text,
        color,
        bold: false,
        face: TextFace::Small,
    }
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
    area: (f32, f32, f32),
    input: &OverlayInput,
    scene: &mut OverlayScene,
) -> f32 {
    let (end, room, line) = area;
    let cell = input.ui.icon.width.max(1.0);
    let fits = (room / cell).floor() as usize;
    let kept: Vec<(char, bool)> = match chars.len() <= fits {
        true => chars.to_vec(),
        false if fits < 2 => return end,
        false => ELLIPSIS
            .chars()
            .map(|c| (c, false))
            .chain(chars[chars.len() - (fits - 1)..].iter().copied())
            .collect(),
    };
    if kept.is_empty() {
        return end;
    }
    let start = (end - kept.len() as f32 * cell).round();
    let mut offset = 0;
    for (text, matched) in group(&kept) {
        let count = text.chars().count();
        scene.text.push(TextRun {
            x: start + offset as f32 * cell,
            baseline: line,
            text,
            color: match matched {
                true => FOREGROUND,
                false => QUIET,
            },
            bold: false,
            face: TextFace::Icon,
        });
        offset += count;
    }
    start
}
