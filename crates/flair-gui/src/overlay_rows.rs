use flair_core::theme::Rgb;

use crate::{
    chrome_bar::{baseline, cap_centre, ellipsize, fill, icon_text, rounded, ui_text},
    font::PROMPT_ICON_SCALE,
    overlay_layout::{OverlayInput, OverlayScene, Row, Sheet, inside, visible},
    renderer::{PxRect, TextFace, TextRun},
    ui::{
        CARET_GAP, DEPTH_INDENT, ELLIPSIS, ITEM_GAP, LINE, NUMBER_COLUMN, PALETTE_FIELD,
        PANEL_PAD_X, PROCESS_ICON_GAP, ROW_PAD, ROW_RADIUS, SELECT_BAR, SELECT_INSET,
        SESSION_NAME_MAX, px,
    },
};

pub(crate) fn head(
    sheet: &Sheet,
    panel: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let band = PxRect {
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: px(PALETTE_FIELD, scale),
    };
    let inset = px(PANEL_PAD_X, scale) + px(ROW_PAD, scale);
    let left = band.x + inset;
    let right = band.x + band.w - inset;
    let (shown, total) = sheet.count;
    let count = match &sheet.field {
        Some(field) if !field.text.is_empty() => format!("{shown} of {total}"),
        Some(_) | None => total.to_string(),
    };
    let count_width = measure(&count, false) * input.ui.small;
    scene.text.push(small(
        (right - count_width).round(),
        baseline(band, input.ui),
        count,
        surfaces.muted,
    ));
    let room = (right - count_width - px(ITEM_GAP, scale) - left).max(0.0);
    match (&sheet.field, sheet.title) {
        (Some(field), _) => {
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
                left + offset,
                baseline(band, input.ui),
                text,
                color,
                false,
            ));
            scene.quads.push(fill(
                PxRect {
                    x: (left + caret).round(),
                    y: (band.y + (band.h - input.ui.height) / 2.0).round(),
                    w: px(LINE, scale),
                    h: input.ui.height.round(),
                },
                theme.accent,
            ));
        }
        (None, Some(title)) => scene.text.push(ui_text(
            left,
            baseline(band, input.ui),
            ellipsize(title, room, true, measure),
            theme.foreground,
            true,
        )),
        (None, None) => {}
    }
    scene.quads.push(fill(
        PxRect {
            x: panel.x,
            y: band.y + band.h,
            w: panel.w,
            h: px(LINE, scale),
        },
        surfaces.outline,
    ));
}

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
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let hovered = input.hover.is_some_and(|point| inside(rect, point));
    match (selected, hovered) {
        (true, _) => {
            scene
                .quads
                .push(rounded(rect, surfaces.accent_soft, ROW_RADIUS, scale));
            let inset = px(SELECT_INSET, scale);
            scene.quads.push(fill(
                PxRect {
                    x: rect.x,
                    y: rect.y + inset,
                    w: px(SELECT_BAR, scale),
                    h: (rect.h - inset * 2.0).max(0.0),
                },
                theme.accent,
            ));
        }
        (false, true) => scene
            .quads
            .push(rounded(rect, surfaces.surface2, ROW_RADIUS, scale)),
        (false, false) => {}
    }
    let pad = px(ROW_PAD, scale);
    let mut x = rect.x + pad + f32::from(row.depth) * px(DEPTH_INDENT, scale);
    let line = baseline(rect, input.ui);
    if let Some(number) = row.number {
        scene
            .text
            .push(small(x.round(), line, number.to_string(), surfaces.faint));
        x += px(NUMBER_COLUMN, scale);
    }
    if gutter {
        if let Some(icon) = row.icon {
            scene.text.push(icon_text(
                x,
                cap_centre(rect, input.ui),
                icon,
                surfaces.muted,
                input.metrics,
            ));
        }
        x += (input.metrics.width * PROMPT_ICON_SCALE).round() + px(PROCESS_ICON_GAP, scale);
    }
    let end = rect.x + rect.w - pad;
    let label = row
        .label
        .map(|label| (label, measure(label, false) * input.ui.small));
    let reserved = label.map_or(0.0, |(_, width)| width + pad);
    let room = (end - x - reserved).max(0.0);
    let name_room = match row.detail {
        Some(_) => room.min(px(SESSION_NAME_MAX, scale)),
        None => room,
    };
    let base = match (row.name_matches.is_empty(), selected) {
        (true, true) => theme.foreground,
        (true, false) | (false, _) => surfaces.bright,
    };
    let name = segments(&marked(&row.name, &row.name_matches), name_room, measure);
    for (text, matched) in name {
        let width = measure(&text, matched);
        let color = match matched {
            true => theme.foreground,
            false => base,
        };
        scene
            .text
            .push(ui_text(x.round(), line, text, color, matched));
        x += width;
    }
    let detail_room = (end - x - pad - reserved).max(0.0);
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
            true => start - pad - width,
            false => end - width,
        };
        scene
            .text
            .push(small(at.round(), line, label.to_owned(), surfaces.muted));
    }
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

fn marked(text: &str, matches: &[usize]) -> Vec<(char, bool)> {
    text.chars()
        .enumerate()
        .filter(|(_, c)| !c.is_control())
        .map(|(index, c)| (c, matches.binary_search(&index).is_ok()))
        .collect()
}

fn segments(
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
    let surfaces = input.theme.surfaces();
    let mut offset = 0;
    for (text, matched) in group(&kept) {
        let count = text.chars().count();
        scene.text.push(TextRun {
            x: start + offset as f32 * cell,
            baseline: line,
            text,
            color: match matched {
                true => surfaces.bright,
                false => surfaces.muted,
            },
            bold: false,
            face: TextFace::Icon,
        });
        offset += count;
    }
    start
}
