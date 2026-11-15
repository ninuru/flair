use flair_core::{config::Side, theme::Rgb};

use crate::{
    agents_layout,
    chrome_bar::{
        Scene, baseline, cap_centre, ellipsize, fill, icon_text, rounded, small_text, ui_text,
    },
    chrome_layout::{ChromeInput, Hit, HitTarget, PanelView, Slot},
    explorer_layout,
    font::PROMPT_ICON_SCALE,
    isles::icon_width,
    overlay_layout::inside,
    panels::Drag,
    renderer::{PxRect, TextRun},
    status::TabParts,
    ui::{
        ADD_KEY, BELL_KEY, CAP_HEIGHT, CAP_RADIUS, CLOSE_KEY, EDGE_GRAB, INSET, ITEM_GAP,
        ITEM_HEIGHT, ITEM_PAD, LINE, NUMBER_COLUMN, PANEL_ROW, PROCESS_ICON_GAP, ROW_GAP, ROW_PAD,
        ROW_RADIUS, SELECT_BAR, SELECT_INSET, SWITCHER, UNDERLINE, UNDERLINE_INSET, px,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabRows {
    pub top: f32,
    pub pitch: f32,
    pub height: f32,
    pub page: usize,
}

pub(crate) fn content(slot: Slot, scale: f32) -> PxRect {
    let line = px(LINE, scale);
    let rect = slot.rect;
    let x = match slot.side {
        Side::Left => rect.x,
        Side::Right => rect.x + line,
    };
    PxRect {
        x,
        w: (rect.w - line).max(0.0),
        ..rect
    }
}

pub(crate) fn body_top(slot: Slot, scale: f32) -> f32 {
    let band = match slot.shared {
        true => px(SWITCHER, scale) + px(LINE, scale),
        false => 0.0,
    };
    slot.rect.y + band + px(INSET, scale)
}

pub(crate) fn tab_rows(slot: Slot, scale: f32) -> TabRows {
    let area = content(slot, scale);
    let top = body_top(slot, scale);
    let bottom = area.y + area.h - px(INSET, scale);
    let height = px(PANEL_ROW, scale);
    let pitch = height + px(ROW_GAP, scale);
    let page = match bottom - top >= height {
        true => ((bottom - top - height) / pitch).floor() as usize + 1,
        false => 0,
    };
    TabRows {
        top,
        pitch,
        height,
        page,
    }
}

pub(crate) fn draw(
    input: &ChromeInput,
    slot: Slot,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> usize {
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    let line = px(LINE, scale);
    let rect = slot.rect;
    scene.quads.push(fill(rect, input.theme.background));
    let hairline = match slot.side {
        Side::Left => rect.x + rect.w - line,
        Side::Right => rect.x,
    };
    scene.quads.push(fill(
        PxRect {
            x: hairline,
            w: line,
            ..rect
        },
        surfaces.outline,
    ));
    let grab = px(EDGE_GRAB, scale);
    scene.hits.push(Hit {
        rect: PxRect {
            x: (hairline + (line - grab) / 2.0).round(),
            w: grab,
            ..rect
        },
        target: HitTarget::Edge(slot.side),
    });
    if slot.shared {
        switcher(input, slot, scene, measure);
    }
    let rows = match slot.view {
        PanelView::Tabs => tabs(input, slot, scene, measure),
        PanelView::Explorer => explorer_layout::draw(input, slot, scene, measure),
        PanelView::Agents => agents_layout::draw(input, slot, scene, measure),
    };
    scene.hits.push(Hit {
        rect,
        target: HitTarget::PanelBody,
    });
    rows
}

fn switcher(
    input: &ChromeInput,
    slot: Slot,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let area = content(slot, scale);
    let band = PxRect {
        h: px(SWITCHER, scale),
        ..area
    };
    let pad = px(ITEM_PAD, scale);
    let height = px(ITEM_HEIGHT, scale);
    let mut x = area.x + px(INSET, scale);
    let end = area.x + area.w - px(INSET, scale);
    for (view, label) in [
        (PanelView::Tabs, "Windows"),
        (PanelView::Explorer, "Files"),
        (PanelView::Agents, "Agents"),
    ] {
        let width = (measure(label, false) + pad * 2.0).round();
        if x.round() + width > end {
            break;
        }
        let item = PxRect {
            x: x.round(),
            y: (band.y + (band.h - height) / 2.0).round(),
            w: width,
            h: height,
        };
        let hit = PxRect {
            y: band.y,
            h: band.h,
            ..item
        };
        let color = if slot.view == view {
            theme.foreground
        } else if input.hover.is_some_and(|point| inside(hit, point)) {
            surfaces.bright
        } else {
            surfaces.muted
        };
        scene.text.push(ui_text(
            item.x + pad,
            baseline(item, input.ui),
            label.to_owned(),
            color,
            false,
        ));
        if slot.view == view {
            let inset = px(UNDERLINE_INSET, scale);
            let thick = px(UNDERLINE, scale);
            scene.quads.push(fill(
                PxRect {
                    x: item.x + inset,
                    y: band.y + band.h - thick,
                    w: (item.w - inset * 2.0).max(0.0),
                    h: thick,
                },
                theme.accent,
            ));
        }
        scene.hits.push(Hit {
            rect: hit,
            target: HitTarget::Switch(view),
        });
        x += width + px(ITEM_GAP, scale);
    }
    scene.quads.push(fill(
        PxRect {
            y: band.y + band.h,
            h: px(LINE, scale),
            ..area
        },
        surfaces.outline,
    ));
}

fn tabs(
    input: &ChromeInput,
    slot: Slot,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> usize {
    let scale = input.scale;
    let area = content(slot, scale);
    let rows = tab_rows(slot, scale);
    let tabs = &input.status.tabs;
    let count = tabs.len();
    let (scroll, drag) = input
        .panels
        .map_or((0, None), |panels| (panels.tabs_scroll, panels.drag));
    let first = scroll.min((count + 1).saturating_sub(rows.page));
    let dragged = match drag {
        Some(Drag::Tab {
            window,
            moved: true,
            ..
        }) => Some(window),
        Some(Drag::Tab { .. } | Drag::Edge { .. }) | None => None,
    };
    let hover = input.hover.filter(|_| dragged.is_none());
    let inset = px(INSET, scale);
    let row_rect = |item: usize| PxRect {
        x: area.x + inset,
        y: rows.top + (item - first) as f32 * rows.pitch,
        w: (area.w - inset * 2.0).max(0.0),
        h: rows.height,
    };
    for item in (first..=count).take(rows.page) {
        let rect = row_rect(item);
        let hovered = hover.is_some_and(|point| inside(rect, point));
        match tabs.get(item) {
            Some(tab) => {
                let state = RowState {
                    hovered,
                    dragged: dragged == Some(tab.window),
                };
                tab_row(input, tab, rect, state, scene, measure);
            }
            None => plus_row(input, rect, hovered, scene),
        }
    }
    if let (Some(window), Some(panels), Some(point)) = (dragged, input.panels, input.hover) {
        let from = tabs.iter().position(|tab| tab.window == window);
        let to = usize::from(panels.drop_index(&input.frame, point.1, count, scale)) - 1;
        if let Some(from) = from.filter(|from| *from != to)
            && (first..first + rows.page).contains(&to)
        {
            let rect = row_rect(to);
            let thick = px(UNDERLINE, scale);
            let gap = rows.pitch - rows.height;
            let y = match to < from {
                true => rect.y - gap / 2.0,
                false => rect.y + rect.h + gap / 2.0,
            };
            scene.quads.push(fill(
                PxRect {
                    y: (y - thick / 2.0).round(),
                    h: thick,
                    ..rect
                },
                input.theme.accent,
            ));
        }
    }
    rows.page
}

#[derive(Clone, Copy)]
struct RowState {
    hovered: bool,
    dragged: bool,
}

fn tab_row(
    input: &ChromeInput,
    tab: &TabParts,
    rect: PxRect,
    state: RowState,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    if state.dragged {
        scene
            .quads
            .push(rounded(rect, surfaces.surface3, ROW_RADIUS, scale));
    } else if tab.active {
        scene
            .quads
            .push(rounded(rect, surfaces.accent_soft, ROW_RADIUS, scale));
    } else if state.hovered {
        scene
            .quads
            .push(rounded(rect, surfaces.surface2, ROW_RADIUS, scale));
    }
    if tab.active {
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
    let mut x = rect.x + px(ROW_PAD, scale);
    scene.text.push(small_text(
        x.round(),
        rect,
        tab.index.to_string(),
        surfaces.faint,
        input.ui,
    ));
    x += px(NUMBER_COLUMN, scale);
    if let Some(icon) = tab.icon {
        let color = match tab.active {
            true => surfaces.bright,
            false => surfaces.muted,
        };
        scene.text.push(icon_text(
            x,
            cap_centre(rect, input.ui),
            icon,
            color,
            input.metrics,
        ));
    }
    x += icon_column(input);
    let square = trailing(rect, scale);
    let name = ellipsize(
        &tab.name,
        square.x - px(ITEM_GAP, scale) - x,
        false,
        measure,
    );
    let color = if tab.active || tab.activity {
        theme.foreground
    } else if state.hovered {
        surfaces.bright
    } else {
        surfaces.muted
    };
    if !name.is_empty() {
        scene.text.push(ui_text(
            x.round(),
            baseline(rect, input.ui),
            name,
            color,
            false,
        ));
    }
    let on_close = input.hover.is_some_and(|point| inside(square, point));
    if state.hovered && on_close {
        scene
            .quads
            .push(rounded(square, surfaces.surface3, CAP_RADIUS, scale));
    }
    let glyph = match (state.hovered, tab.bell) {
        (true, _) if on_close => Some((CLOSE_KEY, theme.foreground)),
        (true, _) => Some((CLOSE_KEY, surfaces.muted)),
        (false, true) => Some((BELL_KEY, surfaces.attention)),
        (false, false) => None,
    };
    if let Some((glyph, color)) = glyph {
        scene.text.push(centred(input, square, glyph, color));
    }
    scene.hits.push(Hit {
        rect: square,
        target: HitTarget::CloseTab(tab.window),
    });
    scene.hits.push(Hit {
        rect,
        target: HitTarget::Tab(tab.window),
    });
}

fn plus_row(input: &ChromeInput, rect: PxRect, hovered: bool, scene: &mut Scene) {
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    let color = match hovered {
        true => {
            scene
                .quads
                .push(rounded(rect, surfaces.surface2, ROW_RADIUS, scale));
            surfaces.bright
        }
        false => surfaces.muted,
    };
    let x = rect.x + px(ROW_PAD, scale) + px(NUMBER_COLUMN, scale);
    scene.text.push(icon_text(
        x,
        cap_centre(rect, input.ui),
        ADD_KEY,
        color,
        input.metrics,
    ));
    scene.text.push(ui_text(
        (x + icon_column(input)).round(),
        baseline(rect, input.ui),
        "New window".to_owned(),
        color,
        false,
    ));
    scene.hits.push(Hit {
        rect,
        target: HitTarget::NewTab,
    });
}

pub(crate) fn icon_column(input: &ChromeInput) -> f32 {
    (input.metrics.width * PROMPT_ICON_SCALE).round() + px(PROCESS_ICON_GAP, input.scale)
}

fn trailing(rect: PxRect, scale: f32) -> PxRect {
    let size = px(CAP_HEIGHT, scale);
    PxRect {
        x: (rect.x + rect.w - px(ROW_PAD, scale) - size).round(),
        y: (rect.y + (rect.h - size) / 2.0).round(),
        w: size,
        h: size,
    }
}

fn centred(input: &ChromeInput, square: PxRect, glyph: char, color: Rgb) -> TextRun {
    icon_text(
        square.x + (square.w - icon_width(glyph, input.metrics)) / 2.0,
        square.y + square.h / 2.0,
        glyph,
        color,
        input.metrics,
    )
}

#[cfg(test)]
#[path = "panel_layout_tests.rs"]
mod tests;
