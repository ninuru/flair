use flair_core::settings::Section;

use crate::{
    chrome_bar::{baseline, ellipsize, fill, opaque, rounded, ui_text},
    overlay_layout::{Field, OverlayInput, OverlayScene, inside, shadow, visible},
    renderer::{Border, PxRect, Quad},
    settings_rows::{entry_height, entry_row, glyph},
    t3_style::{
        BACKGROUND, CARD, CARD_BORDER, CARD_RADIUS, CLOSE_ICON, CLOSE_SIZE, CONTAINER_MAX,
        CONTENT_MIN, CONTENT_PAD_BOTTOM, CONTENT_PAD_TOP, CONTENT_PAD_X, CRUMB_GAP, DIM, DIVIDER,
        FOREGROUND, GHOST_HOVER, GHOST_RADIUS, HAIRLINE, HEADER_HEIGHT, HEADER_PAD_LEFT,
        HEADER_PAD_RIGHT, HEADING_GAP, HEADING_HEIGHT, HEADING_PAD, INSET, INSET_SMALL, MUTED,
        PANEL_BORDER, PANEL_MAX_H, PANEL_MAX_W, PANEL_RADIUS, RAIL_MIN_CONTENT, RAIL_WIDTH,
        SECTION_GAP, SIDEBAR_MIN_CONTENT, SIDEBAR_WIDTH, SMALL_HEIGHT, SMALL_WIDTH,
    },
    ui::{LINE, px},
};

const TITLE: &str = "Settings";
const SLASH: &str = "/";
const OFF: &str = " off";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSheet {
    pub sections: Vec<Section>,
    pub section: Option<usize>,
    pub sections_focused: bool,
    pub crumb: &'static str,
    pub search: Field,
    pub groups: Vec<Group>,
    pub focus: Option<usize>,
    pub dropdown: Option<ChoiceMenu>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceMenu {
    pub item: usize,
    pub options: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub heading: Option<&'static str>,
    pub disabled: bool,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Setting {
        title: &'static str,
        description: &'static str,
        modified: bool,
        control: Control,
        error: Option<String>,
        pending: bool,
    },
    Binding {
        chord: String,
        command: String,
        unbound: bool,
        table: &'static str,
        disabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Toggle(bool),
    Number(String),
    Choice(String),
    Text(String),
    Color(String),
    Editor(Field),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsHit {
    PromptEditor(crate::prompt_editor::PromptAction),
    Section(usize),
    Close,
    Search,
    Toggle(usize),
    Decrease(usize),
    Increase(usize),
    Cycle(usize),
    Choose(usize),
    DismissDropdown,
    Edit(usize),
    Reset(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Sidebar,
    Rail,
    Hidden,
}

struct Frame {
    window: PxRect,
    panel: PxRect,
    nav: Nav,
    header: PxRect,
    viewport: PxRect,
}

struct Placed<'a> {
    group: usize,
    line: Line<'a>,
    top: f32,
    height: f32,
}

enum Line<'a> {
    Heading(&'static str, bool),
    Entry(usize, &'a Entry),
}

pub fn layout(
    sheet: &SettingsSheet,
    input: &OverlayInput,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> OverlayScene {
    let frame = frame(input);
    let mut scene = OverlayScene::default();
    scene.hits.panel = frame.panel;
    backdrop(&frame, input, &mut scene);
    let scale = input.scale;
    if frame.panel.w < px(HEADER_HEIGHT * 2.0, scale) || frame.panel.h < px(HEADER_HEIGHT, scale) {
        return scene;
    }
    nav::layout(sheet, &frame, input, &mut scene, measure);
    if frame.viewport.w >= px(CONTENT_MIN, scale) {
        content(sheet, &frame, input, &mut scene, measure);
    }
    header(sheet, &frame, input, &mut scene, measure);
    dropdown_layout::layout(sheet, frame.window, input, &mut scene, measure);
    scene
}

fn frame(input: &OverlayInput) -> Frame {
    let scale = input.scale;
    let (width, height) = (input.px.0 as f32, input.px.1 as f32);
    let window = PxRect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: height,
    };
    let small = width < px(SMALL_WIDTH, scale) || height < px(SMALL_HEIGHT, scale);
    let inset = px(if small { INSET_SMALL } else { INSET }, scale);
    let w = px(PANEL_MAX_W, scale).min(width - inset * 2.0).max(0.0);
    let h = px(PANEL_MAX_H, scale).min(height - inset * 2.0).max(0.0);
    let panel = PxRect {
        x: ((width - w) / 2.0).round(),
        y: ((height - h) / 2.0).round(),
        w,
        h,
    };
    let nav = if w >= px(SIDEBAR_WIDTH + SIDEBAR_MIN_CONTENT, scale) {
        Nav::Sidebar
    } else if w >= px(RAIL_WIDTH + RAIL_MIN_CONTENT, scale) {
        Nav::Rail
    } else {
        Nav::Hidden
    };
    let nav_width = nav_width(nav, scale);
    let column = PxRect {
        x: panel.x + nav_width,
        w: panel.w - nav_width,
        ..panel
    };
    let header = PxRect {
        h: px(HEADER_HEIGHT, scale).min(panel.h),
        ..column
    };
    let container = px(CONTAINER_MAX, scale).min(column.w);
    let pad = px(CONTENT_PAD_X, scale);
    let top = header.y + header.h + px(CONTENT_PAD_TOP, scale);
    let bottom = panel.y + panel.h - px(CONTENT_PAD_BOTTOM, scale);
    let viewport = PxRect {
        x: (column.x + (column.w - container) / 2.0).round() + pad,
        y: top,
        w: (container - pad * 2.0).max(0.0),
        h: (bottom - top).max(0.0),
    };
    Frame {
        window,
        panel,
        nav,
        header,
        viewport,
    }
}

fn nav_width(nav: Nav, scale: f32) -> f32 {
    match nav {
        Nav::Sidebar => px(SIDEBAR_WIDTH, scale),
        Nav::Rail => px(RAIL_WIDTH, scale),
        Nav::Hidden => 0.0,
    }
}

pub(crate) fn editor_rect(input: &OverlayInput) -> PxRect {
    frame(input).viewport
}

fn backdrop(frame: &Frame, input: &OverlayInput, scene: &mut OverlayScene) {
    let scale = input.scale;
    scene.quads.push(Quad {
        rect: frame.window,
        color: DIM,
        radius: 0.0,
        border: None,
    });
    let panel = frame.panel;
    if panel.w <= 0.0 || panel.h <= 0.0 {
        return;
    }
    shadow(panel, frame.window, scale, scene);
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(PANEL_BORDER),
            width: px(LINE, scale),
        }),
        ..rounded(panel, BACKGROUND, PANEL_RADIUS, scale)
    });
}

fn place(sheet: &SettingsSheet, scale: f32) -> Vec<Placed<'_>> {
    let mut placed = Vec::new();
    let mut top = 0.0;
    let mut item = 0;
    for (group, entries) in sheet.groups.iter().enumerate() {
        if group > 0 {
            top += px(SECTION_GAP, scale);
        }
        let height = px(HEADING_HEIGHT, scale);
        placed.push(Placed {
            group,
            line: Line::Heading(entries.heading.unwrap_or(sheet.crumb), entries.disabled),
            top,
            height,
        });
        top += height + px(HEADING_GAP, scale);
        for entry in &entries.entries {
            let height = entry_height(entry, scale);
            placed.push(Placed {
                group,
                line: Line::Entry(item, entry),
                top,
                height,
            });
            top += height;
            item += 1;
        }
    }
    placed
}

fn offset(placed: &[Placed], focus: Option<usize>, room: f32) -> f32 {
    placed
        .iter()
        .find(|line| match line.line {
            Line::Heading(..) => false,
            Line::Entry(item, _) => Some(item) == focus,
        })
        .map_or(0.0, |line| (line.top + line.height - room).max(0.0))
}

fn header(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let rect = frame.header;
    let line = px(LINE, scale);
    scene.quads.push(fill(
        PxRect {
            x: rect.x,
            y: rect.y + rect.h - line,
            w: (rect.w - line).max(0.0),
            h: line,
        },
        HAIRLINE,
    ));
    let size = px(CLOSE_SIZE, scale);
    let close = PxRect {
        x: rect.x + rect.w - px(HEADER_PAD_RIGHT, scale) - size,
        y: (rect.y + (rect.h - size) / 2.0).round(),
        w: size,
        h: size,
    };
    if input.hover.is_some_and(|point| inside(close, point)) {
        scene
            .quads
            .push(rounded(close, GHOST_HOVER, GHOST_RADIUS, scale));
    }
    scene
        .text
        .push(glyph(CLOSE_ICON, close, MUTED, input.metrics));
    scene.hits.controls.push((SettingsHit::Close, close));
    let end = close.x - px(CRUMB_GAP, scale);
    let base = baseline(rect, input.ui);
    let gap = px(CRUMB_GAP, scale);
    let mut x = rect.x + px(HEADER_PAD_LEFT, scale);
    let query = match frame.nav {
        Nav::Sidebar => None,
        Nav::Rail | Nav::Hidden => Some(&sheet.search).filter(|field| !field.text.is_empty()),
    };
    for (text, color, last) in [
        (TITLE, MUTED, false),
        (SLASH, MUTED, false),
        (sheet.crumb, FOREGROUND, true),
    ] {
        if last && let Some(field) = query {
            let (shown, caret) = visible(&field.text, field.cursor, (end - x).max(0.0), measure);
            scene
                .text
                .push(ui_text(x.round(), base, shown, FOREGROUND, false));
            scene.quads.push(fill(
                PxRect {
                    x: (x + caret).round(),
                    y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
                    w: line,
                    h: input.ui.height.round(),
                },
                FOREGROUND,
            ));
            break;
        }
        let shown = ellipsize(text, end - x, false, measure);
        if shown.is_empty() {
            break;
        }
        let width = measure(&shown, false);
        scene
            .text
            .push(ui_text(x.round(), base, shown, color, false));
        if last {
            break;
        }
        x += width + gap;
    }
}

fn content(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let viewport = frame.viewport;
    let placed = place(sheet, scale);
    let offset = offset(&placed, sheet.focus, viewport.h);
    let shown: Vec<(&Placed, PxRect)> = placed
        .iter()
        .filter(|line| line.top >= offset && line.top + line.height <= offset + viewport.h)
        .map(|line| {
            let rect = PxRect {
                y: viewport.y + line.top - offset,
                h: line.height,
                ..viewport
            };
            (line, rect)
        })
        .collect();
    let cards: Vec<Option<(PxRect, PxRect)>> = (0..sheet.groups.len())
        .map(|group| card(group, &placed, &shown, scale))
        .collect();
    for (card, rows) in cards.iter().flatten() {
        scene.quads.push(Quad {
            border: Some(Border {
                color: opaque(CARD_BORDER),
                width: px(LINE, scale),
            }),
            ..rounded(*card, CARD, CARD_RADIUS, scale)
        });
        let bottom = rows.y + rows.h;
        for strip in [
            PxRect {
                h: rows.y - card.y,
                ..*card
            },
            PxRect {
                y: bottom,
                h: card.y + card.h - bottom,
                ..*card
            },
        ] {
            if strip.h > 0.0 {
                scene.quads.push(fill(strip, BACKGROUND));
            }
        }
    }
    let line = px(LINE, scale);
    for (placed, rect) in shown {
        match placed.line {
            Line::Heading(heading, disabled) => {
                heading_line(heading, disabled, rect, input, scene, measure)
            }
            Line::Entry(item, entry) => {
                if cards[placed.group].is_some_and(|(_, rows)| rows.y < rect.y) {
                    scene.quads.push(fill(
                        PxRect {
                            x: rect.x + line,
                            y: rect.y,
                            w: rect.w - line * 2.0,
                            h: line,
                        },
                        DIVIDER,
                    ));
                }
                let focused = !sheet.sections_focused && sheet.focus == Some(item);
                let open = sheet
                    .dropdown
                    .as_ref()
                    .is_some_and(|menu| menu.item == item);
                entry_row(entry, item, rect, (focused, open), input, scene, measure);
                scene.hits.rows.push((item, rect));
            }
        }
    }
}

fn entry(line: &Placed) -> bool {
    match line.line {
        Line::Heading(..) => false,
        Line::Entry(..) => true,
    }
}

fn card(
    group: usize,
    placed: &[Placed],
    shown: &[(&Placed, PxRect)],
    scale: f32,
) -> Option<(PxRect, PxRect)> {
    let mut rows = shown
        .iter()
        .filter(|(line, _)| line.group == group && entry(line));
    let (first_line, first) = *rows.next()?;
    let (last_line, last) = rows.next_back().copied().unwrap_or((first_line, first));
    let entries = || {
        placed
            .iter()
            .filter(|line| line.group == group && entry(line))
    };
    let radius = px(CARD_RADIUS, scale);
    let above = match entries().any(|line| line.top < first_line.top) {
        true => radius,
        false => 0.0,
    };
    let below = match entries().any(|line| line.top > last_line.top) {
        true => radius,
        false => 0.0,
    };
    let rows = PxRect {
        h: last.y + last.h - first.y,
        ..first
    };
    Some((
        PxRect {
            y: rows.y - above,
            h: rows.h + above + below,
            ..rows
        },
        rows,
    ))
}

fn heading_line(
    heading: &str,
    disabled: bool,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let x = rect.x + px(HEADING_PAD, input.scale);
    let room = rect.x + rect.w - x;
    let line = baseline(rect, input.ui);
    let shown = ellipsize(heading, room, true, measure);
    let end = x + measure(&shown, true);
    scene.text.push(ui_text(x, line, shown, FOREGROUND, true));
    if disabled {
        scene.text.push(ui_text(
            end.round(),
            line,
            ellipsize(OFF, rect.x + rect.w - end, false, measure),
            MUTED,
            false,
        ));
    }
}

#[cfg(test)]
#[path = "settings_layout_tests.rs"]
mod tests;

#[path = "settings_dropdown_layout.rs"]
mod dropdown_layout;

#[path = "settings_nav.rs"]
mod nav;
