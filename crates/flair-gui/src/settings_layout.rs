use crate::{
    chrome_bar::{baseline, ellipsize, fill, opaque, rounded, ui_text},
    chrome_layout::chrome_bottom,
    overlay_layout::{Field, OverlayInput, OverlayScene, cap, footer, inside, search},
    renderer::{Border, PxRect, Quad},
    settings_rows::{entry_height, entry_row},
    ui::{
        CAP_HEIGHT, CAP_PAD, CONTENT_INSET, CONTENT_WIDTH, CONTROL_MAX, CRUMB_GAP, ENTRY_HEIGHT,
        ESCAPE_KEY, FIELD_GAP, FIELD_HEIGHT, FOOTER_GAP, FOOTER_RULE_GAP, GROUP_GAP, HEADER_GAP,
        HEADING_GAP, ITEM_GAP, ITEM_HEIGHT, LINE, NAV_BREAKPOINT, NAV_NARROW, NAV_PAD_X, NAV_WIDTH,
        PANEL_PAD_Y, PANEL_RADIUS, ROW_GAP, ROW_PAD, ROW_RADIUS, SECTION_HEIGHT, px,
    },
};

const TITLE: &str = "Settings";
const CLOSE: &str = "close";
const OFF: &str = " off";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSheet {
    pub sections: Vec<&'static str>,
    pub section: Option<usize>,
    pub sections_focused: bool,
    pub crumb: &'static str,
    pub search: Field,
    pub groups: Vec<Group>,
    pub focus: Option<usize>,
    pub dropdown: Option<ChoiceMenu>,
    pub keys: Vec<(&'static str, String)>,
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
        source: &'static str,
        control: Control,
        error: Option<String>,
        pending: bool,
    },
    Binding {
        chord: String,
        command: String,
        unbound: bool,
        table: &'static str,
        source: &'static str,
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
}

struct Frame {
    page: PxRect,
    nav: PxRect,
    header: PxRect,
    search: PxRect,
    viewport: PxRect,
    footer: Option<f32>,
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
    scene.hits.panel = frame.page;
    if fits(frame.page, input) {
        scene.quads.push(fill(frame.page, input.theme.background));
    }
    nav(sheet, &frame, input, &mut scene, measure);
    if frame.search.w < px(CONTROL_MAX, input.scale) || !fits(frame.search, input) {
        return scene;
    }
    header(sheet, &frame, input, &mut scene, measure);
    search(&sheet.search, frame.search, input, &mut scene, measure);
    scene
        .hits
        .controls
        .push((SettingsHit::Search, frame.search));
    let hidden = content(sheet, &frame, input, &mut scene, measure);
    if let Some(caps) = frame.footer {
        let scale = input.scale;
        let rule = PxRect {
            x: frame.search.x,
            y: caps - px(FOOTER_RULE_GAP, scale) - px(LINE, scale),
            w: frame.search.w,
            h: px(LINE, scale),
        };
        scene.quads.push(fill(rule, input.theme.surfaces().outline));
        footer(
            &sheet.keys,
            hidden,
            input,
            (rule.x, caps, rule.w),
            &mut scene,
            measure,
        );
    }
    dropdown_layout::layout(sheet, frame.page, input, &mut scene, measure);
    scene
}

fn fits(rect: PxRect, input: &OverlayInput) -> bool {
    rect.w > 0.0
        && rect.h > 0.0
        && rect.x >= 0.0
        && rect.y >= 0.0
        && rect.x + rect.w <= input.px.0 as f32
        && rect.y + rect.h <= input.px.1 as f32
}

fn frame(input: &OverlayInput) -> Frame {
    let scale = input.scale;
    let (width, height) = (input.px.0 as f32, input.px.1 as f32);
    let top = chrome_bottom(scale);
    let page = PxRect {
        x: 0.0,
        y: top,
        w: width,
        h: (height - top).max(0.0),
    };
    let nav_width = match width >= px(NAV_BREAKPOINT, scale) {
        true => px(NAV_WIDTH, scale),
        false => px(NAV_NARROW, scale),
    }
    .min(width);
    let remaining = width - nav_width;
    let w = px(CONTENT_WIDTH, scale)
        .min(remaining - px(CONTENT_INSET, scale) * 2.0)
        .max(0.0);
    let x = nav_width + px(CONTENT_INSET, scale).max(((remaining - w) / 2.0).floor());
    let header = PxRect {
        x,
        y: top + px(PANEL_PAD_Y, scale),
        w,
        h: px(ITEM_HEIGHT, scale),
    };
    let search = PxRect {
        y: header.y + header.h + px(HEADER_GAP, scale),
        h: px(FIELD_HEIGHT, scale),
        ..header
    };
    let viewport_top = search.y + search.h + px(HEADER_GAP, scale);
    let caps = height - px(PANEL_PAD_Y, scale) - px(CAP_HEIGHT, scale);
    let bottom = caps - px(FOOTER_RULE_GAP, scale) * 2.0 - px(LINE, scale);
    let (bottom, footer) = match bottom - viewport_top >= px(ENTRY_HEIGHT, scale) {
        true => (bottom, Some(caps)),
        false => (height - px(PANEL_PAD_Y, scale), None),
    };
    Frame {
        page,
        nav: PxRect {
            w: nav_width,
            ..page
        },
        header,
        search,
        viewport: PxRect {
            y: viewport_top,
            h: (bottom - viewport_top).max(0.0),
            ..header
        },
        footer,
    }
}

pub(crate) fn editor_rect(input: &OverlayInput) -> PxRect {
    frame(input).viewport
}

fn place(sheet: &SettingsSheet, scale: f32) -> Vec<Placed<'_>> {
    let mut placed = Vec::new();
    let mut top = 0.0;
    let mut item = 0;
    for (group, entries) in sheet.groups.iter().enumerate() {
        if group > 0 {
            top += px(GROUP_GAP, scale);
        }
        if let Some(heading) = entries.heading {
            let height = px(ITEM_HEIGHT, scale);
            placed.push(Placed {
                group,
                line: Line::Heading(heading, entries.disabled),
                top,
                height,
            });
            top += height + px(HEADING_GAP, scale);
        }
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

fn nav(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let hairline = PxRect {
        x: frame.nav.w,
        w: px(LINE, scale),
        ..frame.nav
    };
    if fits(hairline, input) {
        scene.quads.push(fill(hairline, surfaces.outline));
    }
    let pad = px(NAV_PAD_X, scale);
    let width = (frame.nav.w - pad * 2.0).max(0.0);
    let title = PxRect {
        x: pad,
        y: frame.nav.y + px(PANEL_PAD_Y, scale),
        w: width,
        h: px(ITEM_HEIGHT, scale),
    };
    if !fits(title, input) {
        return;
    }
    scene.text.push(ui_text(
        title.x,
        baseline(title, input.ui),
        ellipsize(TITLE, width, true, measure),
        theme.foreground,
        true,
    ));
    let top = title.y + title.h + px(FIELD_GAP, scale);
    let pitch = px(SECTION_HEIGHT, scale) + px(ROW_GAP, scale);
    let text_pad = px(ROW_PAD, scale);
    for (index, name) in sheet.sections.iter().enumerate() {
        let rect = PxRect {
            y: top + index as f32 * pitch,
            h: px(SECTION_HEIGHT, scale),
            ..title
        };
        if !fits(rect, input) {
            break;
        }
        let hovered = input.hover.is_some_and(|point| inside(rect, point));
        let fill_color = match (sheet.section == Some(index), hovered) {
            (true, _) => Some(surfaces.surface3),
            (false, true) => Some(surfaces.surface2),
            (false, false) => None,
        };
        if let Some(color) = fill_color {
            scene.quads.push(rounded(rect, color, ROW_RADIUS, scale));
        }
        scene.text.push(ui_text(
            rect.x + text_pad,
            baseline(rect, input.ui),
            ellipsize(name, width - text_pad * 2.0, false, measure),
            theme.foreground,
            false,
        ));
        scene
            .hits
            .controls
            .push((SettingsHit::Section(index), rect));
    }
}

fn header(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let muted = theme.surfaces().muted;
    let rect = frame.header;
    let line = baseline(rect, input.ui);
    let right = rect.x + rect.w;
    let close_x = (right - measure(CLOSE, false)).round();
    let cap_width = (measure(ESCAPE_KEY, false) * input.ui.small + px(CAP_PAD, scale) * 2.0)
        .max(px(CAP_HEIGHT, scale))
        .round();
    let cap_x = close_x - px(ITEM_GAP, scale) - cap_width;
    cap(ESCAPE_KEY, (cap_x, rect), input, scene, measure);
    scene
        .text
        .push(ui_text(close_x, line, CLOSE.to_owned(), muted, false));
    scene.hits.controls.push((
        SettingsHit::Close,
        PxRect {
            x: cap_x,
            w: right - cap_x,
            ..rect
        },
    ));
    let end = cap_x - px(FOOTER_GAP, scale);
    let mut x = rect.x;
    for (text, color, bold) in [
        (TITLE, muted, false),
        (CRUMB_GAP, muted, false),
        (sheet.crumb, theme.foreground, true),
    ] {
        let shown = ellipsize(text, end - x, bold, measure);
        if shown.is_empty() {
            break;
        }
        let width = measure(&shown, bold);
        scene
            .text
            .push(ui_text(x.round(), line, shown, color, bold));
        x += width;
    }
}

fn content(
    sheet: &SettingsSheet,
    frame: &Frame,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> usize {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let viewport = frame.viewport;
    if viewport.h < px(ENTRY_HEIGHT, scale) {
        return 0;
    }
    let placed = place(sheet, scale);
    let offset = offset(&placed, sheet.focus, viewport.h);
    let visible =
        |line: &Placed| line.top >= offset && line.top + line.height <= offset + viewport.h;
    let hidden = placed
        .iter()
        .filter(|line| entry(line) && !visible(line))
        .count();
    let shown: Vec<(&Placed, PxRect)> = placed
        .iter()
        .filter(|line| visible(line))
        .map(|line| {
            let rect = PxRect {
                y: viewport.y + line.top - offset,
                h: line.height,
                ..viewport
            };
            (line, rect)
        })
        .collect();
    let panels: Vec<Option<(PxRect, PxRect)>> = (0..sheet.groups.len())
        .map(|group| panel(group, &placed, &shown, scale))
        .collect();
    for (panel, rows) in panels.iter().flatten() {
        scene.quads.push(Quad {
            border: Some(Border {
                color: opaque(surfaces.outline),
                width: px(LINE, scale),
            }),
            ..rounded(*panel, surfaces.surface1, PANEL_RADIUS, scale)
        });
        let bottom = rows.y + rows.h;
        for strip in [
            PxRect {
                h: rows.y - panel.y,
                ..*panel
            },
            PxRect {
                y: bottom,
                h: panel.y + panel.h - bottom,
                ..*panel
            },
        ] {
            if strip.h > 0.0 {
                scene.quads.push(fill(strip, theme.background));
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
                if panels[placed.group].is_some_and(|(_, rows)| rows.y < rect.y) {
                    scene.quads.push(fill(
                        PxRect {
                            x: rect.x + line,
                            y: rect.y,
                            w: rect.w - line * 2.0,
                            h: line,
                        },
                        surfaces.outline,
                    ));
                }
                let focused = !sheet.sections_focused && sheet.focus == Some(item);
                entry_row(entry, item, rect, focused, input, scene, measure);
                scene.hits.rows.push((item, rect));
            }
        }
    }
    hidden
}

fn entry(line: &Placed) -> bool {
    match line.line {
        Line::Heading(..) => false,
        Line::Entry(..) => true,
    }
}

fn panel(
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
    let radius = px(PANEL_RADIUS, scale);
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
    let line = baseline(rect, input.ui);
    let shown = ellipsize(heading, rect.w, true, measure);
    let end = rect.x + measure(&shown, true);
    scene
        .text
        .push(ui_text(rect.x, line, shown, input.theme.foreground, true));
    if disabled {
        scene.text.push(ui_text(
            end.round(),
            line,
            ellipsize(OFF, rect.x + rect.w - end, false, measure),
            input.theme.surfaces().muted,
            false,
        ));
    }
}

#[cfg(test)]
#[path = "settings_layout_tests.rs"]
mod tests;

#[path = "settings_dropdown_layout.rs"]
mod dropdown_layout;
