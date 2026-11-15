use super::*;
use crate::renderer::TextFace;
use crate::t3_style::{
    POPUP, POPUP_BORDER, POPUP_GAP, POPUP_HOVER, POPUP_PAD, POPUP_ROW, POPUP_ROW_PAD,
    POPUP_ROW_RADIUS, POPUP_ROWS, POPUP_SELECTED, SELECT_RADIUS, SHADOW,
};

pub(super) fn layout(
    sheet: &SettingsSheet,
    window: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let Some(menu) = &sheet.dropdown else { return };
    let Some((_, anchor)) = scene
        .hits
        .controls
        .iter()
        .find(|(hit, _)| *hit == SettingsHit::Cycle(menu.item))
    else {
        return;
    };
    let anchor = *anchor;
    let scale = input.scale;
    let gap = px(POPUP_GAP, scale);
    let row = px(POPUP_ROW, scale);
    let pad = px(POPUP_PAD, scale);
    let row_pad = px(POPUP_ROW_PAD, scale);
    let below = window.y + window.h - anchor.y - anchor.h - gap * 2.0;
    let above = anchor.y - window.y - gap * 2.0;
    let down = below >= above;
    let available = if down { below } else { above };
    let capacity = (((available - pad * 2.0) / row).floor().max(0.0) as usize)
        .min(POPUP_ROWS)
        .min(menu.options.len());
    if capacity == 0 {
        return;
    }
    let widest = menu
        .options
        .iter()
        .map(|name| measure(name, false))
        .fold(0.0, f32::max);
    let right = anchor.x + anchor.w;
    let width = (widest + row_pad * 2.0 + pad * 2.0)
        .ceil()
        .max(anchor.w)
        .min(right - window.x);
    let height = capacity as f32 * row + pad * 2.0;
    let panel = PxRect {
        x: right - width,
        y: if down {
            anchor.y + anchor.h + gap
        } else {
            anchor.y - height - gap
        },
        w: width,
        h: height,
    };
    scene.text.retain(|run| {
        let size = match run.face {
            TextFace::Small => input.ui.small,
            TextFace::Ui | TextFace::Mono | TextFace::Icon | TextFace::PromptIcon => 1.0,
        };
        run.baseline + (input.ui.height - input.ui.ascent) * size <= panel.y
            || run.baseline - input.ui.ascent * size >= panel.y + panel.h
            || run.x >= panel.x + panel.w
            || run.x + measure(&run.text, run.bold) * size <= panel.x
    });
    let drop = px(POPUP_GAP * 2.0, scale);
    scene.quads.push(Quad {
        rect: PxRect {
            x: panel.x - drop,
            y: panel.y,
            w: panel.w + drop * 2.0,
            h: panel.h + drop * 2.0,
        },
        color: SHADOW,
        radius: px(SELECT_RADIUS, scale) + drop,
        border: None,
    });
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(POPUP_BORDER),
            width: px(LINE, scale),
        }),
        ..rounded(panel, POPUP, SELECT_RADIUS, scale)
    });
    scene.hits.controls.clear();
    scene.hits.rows.clear();
    let start = menu
        .selected
        .saturating_sub(capacity / 2)
        .min(menu.options.len().saturating_sub(capacity));
    for (index, name) in menu.options.iter().enumerate().skip(start).take(capacity) {
        let rect = PxRect {
            x: panel.x + pad,
            y: panel.y + pad + (index - start) as f32 * row,
            w: panel.w - pad * 2.0,
            h: row,
        };
        let fill_color = match (
            index == menu.selected,
            input.hover.is_some_and(|point| inside(rect, point)),
        ) {
            (true, _) => Some(POPUP_SELECTED),
            (false, true) => Some(POPUP_HOVER),
            (false, false) => None,
        };
        if let Some(color) = fill_color {
            scene
                .quads
                .push(rounded(rect, color, POPUP_ROW_RADIUS, scale));
        }
        scene.text.push(ui_text(
            rect.x + row_pad,
            baseline(rect, input.ui),
            ellipsize(name, rect.w - row_pad * 2.0, false, measure),
            FOREGROUND,
            false,
        ));
        scene.hits.controls.push((SettingsHit::Choose(index), rect));
    }
    scene
        .hits
        .controls
        .push((SettingsHit::DismissDropdown, window));
}
