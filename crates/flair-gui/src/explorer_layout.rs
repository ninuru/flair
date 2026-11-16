use crate::{
    chrome_bar::{
        Scene, baseline, cap_centre, ellipsize, fill, icon_text, opaque, rounded, small_text, tail,
        ui_text,
    },
    chrome_layout::{ChromeInput, Hit, HitTarget, Slot},
    explorer::{RowKind, TreeRow},
    overlay_layout::{OverlayInput, OverlayScene, footer, inside},
    overlay_rows::{marked, segments},
    panel_layout::{body_top, content, icon_column},
    panels::Panels,
    process_icon::file_icon,
    renderer::{Border, PxRect, Quad},
    ui::{
        CAP_HEIGHT, CHEVRON_DOWN, CHEVRON_RIGHT, ELLIPSIS, ENTER_KEY, ESCAPE_KEY, FIELD_RADIUS,
        FOOTER_RULE_GAP, INSET, ITEM_PAD, LINE, PANEL_ROW, ROW_PAD, ROW_RADIUS, SEARCH_KEY,
        SELECT_BAR, SELECT_INSET, TREE_INDENT, TREE_ROW, px,
    },
};

#[derive(Clone, Copy)]
struct RowState {
    selected: bool,
    focused: bool,
    hovered: bool,
}

pub(crate) fn draw(
    input: &ChromeInput,
    slot: Slot,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> usize {
    let Some(panels) = input.panels else {
        return 0;
    };
    let scale = input.scale;
    let area = content(slot, scale);
    let inset = px(INSET, scale);
    let width = (area.w - inset * 2.0).max(0.0);
    let mut top = body_top(slot, scale);
    let mut bottom = area.y + area.h - inset;
    if panels.focused {
        bottom = keys(input, &verbs(panels), (area, bottom), scene, measure);
    }
    let explorer = &panels.explorer;
    if let Some(filter) = explorer.filter() {
        let field = PxRect {
            x: area.x + inset,
            y: top,
            w: width,
            h: px(PANEL_ROW, scale),
        };
        filter_field(input, filter, explorer.editing(), field, scene, measure);
        top += field.h + inset;
    }
    let height = px(TREE_ROW, scale);
    let page = match bottom - top >= height {
        true => ((bottom - top) / height).floor() as usize,
        false => 0,
    };
    let row_rect = |slot: usize| PxRect {
        x: area.x + inset,
        y: top + slot as f32 * height,
        w: width,
        h: height,
    };
    let rows = explorer.rows();
    if rows.is_empty() && page > 0 {
        let rect = row_rect(0);
        scene.text.push(ui_text(
            (rect.x + px(ROW_PAD, scale)).round(),
            baseline(rect, input.ui),
            "no directory".to_owned(),
            input.theme.surfaces().faint,
            false,
        ));
    }
    let first = explorer.scroll();
    let selected = explorer.selected();
    for (index, row) in rows.iter().enumerate().skip(first).take(page) {
        let rect = row_rect(index - first);
        let state = RowState {
            selected: selected == Some(index),
            focused: panels.focused,
            hovered: input.hover.is_some_and(|point| inside(rect, point)),
        };
        tree_row(input, row, rect, state, scene, measure);
        scene.hits.push(Hit {
            rect,
            target: HitTarget::Row(index),
        });
    }
    page
}

fn tree_row(
    input: &ChromeInput,
    row: &TreeRow,
    rect: PxRect,
    state: RowState,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    match (state.selected, state.focused) {
        (true, true) => {
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
        (true, false) => scene
            .quads
            .push(rounded(rect, surfaces.surface3, ROW_RADIUS, scale)),
        (false, _) if state.hovered => {
            scene
                .quads
                .push(rounded(rect, surfaces.surface2, ROW_RADIUS, scale));
        }
        (false, _) => {}
    }
    let column = icon_column(input);
    let start = rect.x + px(ROW_PAD, scale) + f32::from(row.depth) * px(TREE_INDENT, scale);
    let centre = cap_centre(rect, input.ui);
    let end = rect.x + rect.w - px(ROW_PAD, scale);
    let faint = surfaces.faint;
    let open = match &row.kind {
        RowKind::Dir { open } => Some(*open),
        RowKind::File => None,
        RowKind::More(count) => {
            let text = format!("+{count} more");
            scene.text.push(small_text(
                (start + column).round(),
                rect,
                text,
                faint,
                input.ui,
            ));
            return;
        }
        RowKind::Loading => {
            let text = ELLIPSIS.to_owned();
            scene.text.push(ui_text(
                (start + column).round(),
                baseline(rect, input.ui),
                text,
                faint,
                false,
            ));
            return;
        }
        RowKind::Error(error) => {
            let x = (start + column).round();
            let text = ellipsize(error, end - x, false, measure);
            if !text.is_empty() {
                scene
                    .text
                    .push(ui_text(x, baseline(rect, input.ui), text, faint, false));
            }
            return;
        }
    };
    if let Some(open) = open {
        let chevron = match open {
            true => CHEVRON_DOWN,
            false => CHEVRON_RIGHT,
        };
        scene
            .text
            .push(icon_text(start, centre, chevron, faint, input.metrics));
    }
    let icon = file_icon(&row.name, open.is_some(), open == Some(true));
    scene.text.push(icon_text(
        start + column,
        centre,
        icon,
        surfaces.muted,
        input.metrics,
    ));
    let base = match state.selected {
        true => theme.foreground,
        false => surfaces.bright,
    };
    let mut x = (start + column * 2.0).round();
    let line = baseline(rect, input.ui);
    for (text, matched) in segments(&marked(&row.name, &row.matches), end - x, measure) {
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
}

fn filter_field(
    input: &ChromeInput,
    filter: &str,
    editing: bool,
    rect: PxRect,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let line = px(LINE, scale);
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: line,
        }),
        ..rounded(rect, theme.background, FIELD_RADIUS, scale)
    });
    let pad = px(ITEM_PAD, scale);
    let x = rect.x + pad;
    let base = baseline(rect, input.ui);
    let room = (rect.w - pad * 2.0 - line).max(0.0);
    let text = tail(filter, room, false, measure);
    let caret = (x + measure(&text, false)).round();
    match text.is_empty() {
        true => scene
            .text
            .push(ui_text(x, base, "filter".to_owned(), surfaces.faint, false)),
        false => scene
            .text
            .push(ui_text(x, base, text, theme.foreground, false)),
    }
    if editing {
        scene.quads.push(fill(
            PxRect {
                x: caret,
                y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
                w: line,
                h: input.ui.height.round(),
            },
            theme.accent,
        ));
    }
}

fn verbs(panels: &Panels) -> Vec<(&'static str, String)> {
    let explorer = &panels.explorer;
    let folder = explorer
        .selected()
        .and_then(|index| explorer.rows().get(index))
        .is_some_and(|row| matches!(row.kind, RowKind::Dir { .. }));
    match explorer.editing() {
        true => vec![
            (ENTER_KEY, "keep".to_owned()),
            (ESCAPE_KEY, "clear".to_owned()),
        ],
        false => [
            Some((ENTER_KEY, "open")),
            Some(("i", "insert")),
            (folder && panels.at_prompt()).then_some(("c", "cd")),
            Some(("y", "copy")),
            Some((SEARCH_KEY, "filter")),
            Some((ESCAPE_KEY, "back")),
        ]
        .into_iter()
        .flatten()
        .map(|(key, verb)| (key, verb.to_owned()))
        .collect(),
    }
}

pub(crate) fn keys(
    input: &ChromeInput,
    verbs: &[(&'static str, String)],
    room: (PxRect, f32),
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let (area, bottom) = room;
    let scale = input.scale;
    let gap = px(FOOTER_RULE_GAP, scale);
    let line = px(LINE, scale);
    let caps = bottom - px(CAP_HEIGHT, scale);
    let rule = caps - gap - line;
    scene.quads.push(fill(
        PxRect {
            y: rule,
            h: line,
            ..area
        },
        input.theme.surfaces().outline,
    ));
    let overlay = OverlayInput {
        metrics: input.metrics,
        px: input.px,
        scale,
        ui: input.ui,
        theme: input.theme,
        hover: None,
        icons: true,
    };
    let inset = px(INSET, scale);
    let mut drawn = OverlayScene::default();
    footer(
        verbs,
        0,
        &overlay,
        (area.x + inset, caps, (area.w - inset * 2.0).max(0.0)),
        &mut drawn,
        measure,
    );
    scene.quads.extend(drawn.quads);
    scene.text.extend(drawn.text);
    rule - gap
}

#[cfg(test)]
#[path = "explorer_layout_tests.rs"]
mod tests;
