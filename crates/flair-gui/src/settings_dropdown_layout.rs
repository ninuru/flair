use super::*;

pub(super) fn layout(
    sheet: &SettingsSheet,
    page: PxRect,
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
    let gap = px(4.0, scale);
    let row = px(28.0, scale);
    let pad = px(4.0, scale);
    let below = page.y + page.h - anchor.y - anchor.h - gap;
    let above = anchor.y - page.y - gap;
    let down = below >= above;
    let available = if down { below } else { above };
    let capacity = (((available - pad * 2.0) / row).floor().max(0.0) as usize)
        .min(10)
        .min(menu.options.len());
    if capacity == 0 {
        return;
    }
    let height = capacity as f32 * row + pad * 2.0;
    let panel = PxRect {
        x: anchor.x,
        y: if down {
            anchor.y + anchor.h + gap
        } else {
            anchor.y - height - gap
        },
        w: anchor.w,
        h: height,
    };
    let surfaces = input.theme.surfaces();
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: px(LINE, scale),
        }),
        ..rounded(panel, surfaces.surface1, ROW_RADIUS, scale)
    });
    scene.text.retain(|run| {
        run.baseline + input.ui.height - input.ui.ascent <= panel.y
            || run.baseline - input.ui.ascent >= panel.y + panel.h
            || run.x >= panel.x + panel.w
            || run.x + measure(&run.text, run.bold) <= panel.x
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
        let selected = index == menu.selected;
        if selected || input.hover.is_some_and(|point| inside(rect, point)) {
            scene
                .quads
                .push(rounded(rect, surfaces.surface3, ROW_RADIUS, scale));
        }
        scene.text.push(ui_text(
            rect.x + px(ITEM_GAP, scale),
            baseline(rect, input.ui),
            ellipsize(name, rect.w - px(ITEM_GAP * 2.0, scale), false, measure),
            input.theme.foreground,
            false,
        ));
        scene.hits.controls.push((SettingsHit::Choose(index), rect));
    }
    scene
        .hits
        .controls
        .push((SettingsHit::DismissDropdown, page));
}
