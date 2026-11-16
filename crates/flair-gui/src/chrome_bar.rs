use flair_core::{
    agent::AgentState,
    config::IconSide,
    theme::{Rgb, Rgba, Theme, blend},
};

use crate::{
    CellMetrics, UiMetrics, agents_layout,
    chrome_layout::{BarField, ChromeInput, Hit, HitTarget, PanelView, bar_height},
    isles::{icon_baseline, icon_width},
    notice::Notice,
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    status::{BadgeKind, BadgeParts, TabParts},
    t3_style::{
        BADGE_HEIGHT, BADGE_NEUTRAL_TINT, BADGE_PAD, BADGE_RADIUS, BADGE_TINT, BAR_PAD, BAR_TINT,
        BUTTON_HOVER_TINT, BUTTON_RADIUS, BUTTON_SIZE, CRUMB_SLASH, CRUMB_SPACE, TAB_ACTIVE_TINT,
        TAB_GAP, TAB_HEIGHT, TAB_HOVER_TINT, TAB_PAD, TAB_RADIUS,
    },
    ui::{
        BELL_KEY, DOT, ELLIPSIS, FIELD_RADIUS, ITEM_GAP, ITEM_HEIGHT, ITEM_PAD, LINE,
        NOTICE_FIELD_MIN, PROCESS_ICON_GAP, SESSION_NAME_MAX, TAB_NAME_MAX, px,
    },
};

#[derive(Default)]
pub(crate) struct Scene {
    pub quads: Vec<Quad>,
    pub text: Vec<TextRun>,
    pub hits: Vec<Hit>,
}

struct Placed {
    index: usize,
    label: String,
    width: f32,
}

pub(crate) fn bar(
    input: &ChromeInput,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let width = input.px.0 as f32;
    let bar = bar_height(scale);
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: bar,
        },
        input.theme.background,
    ));
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: bar,
            w: width,
            h: px(LINE, scale),
        },
        tint(input.theme, BAR_TINT),
    ));
    let size = px(BUTTON_SIZE, scale);
    let right = width - px(BAR_PAD, scale);
    let indicator = agents_attention(input)
        .filter(|_| !input.zen)
        .map(|color| (bar_item(right - size, size, BUTTON_SIZE, scale), color));
    let right = indicator.map_or(right, |(rect, _)| rect.x - px(ITEM_GAP, scale));
    let end = badges(input, right, scene, measure).map_or(right, |left| left - px(ITEM_GAP, scale));
    let start = session(input, end, scene, measure) + px(CRUMB_SPACE, scale);
    match (input.prompt, input.notice) {
        (Some(prompt), Some(notice)) => split(input, prompt, notice, (start, end), scene, measure),
        (Some(prompt), None) => field(input, prompt, start, end, scene, measure),
        (None, Some(notice)) => {
            let x = start + px(ITEM_PAD, scale);
            note(input, notice, (x, end), scene, measure);
        }
        (None, None) if input.frame.bar_tabs => tabs(input, start, end, scene, measure),
        (None, None) => {}
    }
    if let Some((rect, color)) = indicator {
        icon_button(
            input,
            rect,
            (BELL_KEY, color),
            HitTarget::AgentsButton,
            scene,
        );
    }
}

fn agents_attention(input: &ChromeInput) -> Option<Rgb> {
    let agents = &input.status.agents;
    let visible = input
        .frame
        .slots()
        .any(|slot| slot.view == PanelView::Agents);
    if visible || !agents.iter().any(|agent| agent.unseen) {
        return None;
    }
    let urgent = agents.iter().any(|agent| {
        agent.unseen && matches!(agent.state, AgentState::Blocked | AgentState::Error)
    });
    Some(match urgent {
        true => input.theme.surfaces().attention,
        false => input.theme.foreground,
    })
}

fn split(
    input: &ChromeInput,
    prompt: BarField,
    notice: &Notice,
    room: (f32, f32),
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let (start, end) = room;
    let gap = px(ITEM_GAP, scale);
    let lead = px(DOT, scale) + gap;
    let free = (end - start - px(NOTICE_FIELD_MIN, scale) - gap).floor();
    let width = (lead + measure(&notice.text, false)).ceil().min(free);
    if width < lead + measure(ELLIPSIS, false) {
        field(input, prompt, start, end, scene, measure);
        return;
    }
    field(input, prompt, start, end - width - gap, scene, measure);
    note(input, notice, (end - width, end), scene, measure);
}

fn note(
    input: &ChromeInput,
    notice: &Notice,
    room: (f32, f32),
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let (x, end) = room;
    let surfaces = input.theme.surfaces();
    let dot_color = match notice.error {
        true => surfaces.attention,
        false => surfaces.muted,
    };
    let dot = px(DOT, scale);
    let text_x = (x + dot + px(ITEM_GAP, scale)).round();
    let text = ellipsize(&notice.text, end - text_x, false, measure);
    if text.is_empty() {
        return;
    }
    let dot_rect = bar_item(x, dot, DOT, scale);
    scene
        .quads
        .push(rounded(dot_rect, dot_color, DOT / 2.0, scale));
    let rect = bar_item(x, end - x, ITEM_HEIGHT, scale);
    scene.text.push(ui_text(
        text_x,
        baseline(rect, input.ui),
        text,
        surfaces.muted,
        false,
    ));
}

fn session(
    input: &ChromeInput,
    limit: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let name_x = px(BAR_PAD, scale);
    let space = px(CRUMB_SPACE, scale);
    let slash_width = measure(CRUMB_SLASH, false);
    let room = (limit - name_x - space - slash_width).min(px(SESSION_NAME_MAX, scale));
    let name = ellipsize(&input.status.chip.session, room, false, measure);
    let right = (name_x + measure(&name, false)).round();
    let rect = bar_item(name_x, (right - name_x).max(0.0), TAB_HEIGHT, scale);
    let base = baseline(rect, input.ui);
    scene
        .text
        .push(ui_text(name_x, base, name, input.theme.foreground, false));
    let slash_x = right + space;
    scene.text.push(ui_text(
        slash_x,
        base,
        CRUMB_SLASH.to_owned(),
        input.theme.surfaces().muted,
        false,
    ));
    scene.hits.push(Hit {
        rect: full_height(
            PxRect {
                w: slash_x + slash_width - name_x,
                ..rect
            },
            scale,
        ),
        target: HitTarget::Session,
    });
    slash_x + slash_width
}

fn icon_button(
    input: &ChromeInput,
    rect: PxRect,
    glyph: (char, Rgb),
    target: HitTarget,
    scene: &mut Scene,
) {
    let scale = input.scale;
    let hit = full_height(rect, scale);
    if hovered(input, hit) {
        scene.quads.push(rounded(
            rect,
            tint(input.theme, BUTTON_HOVER_TINT),
            BUTTON_RADIUS,
            scale,
        ));
    }
    let (glyph, color) = glyph;
    scene.text.push(icon_text(
        rect.x + (rect.w - icon_width(glyph, input.metrics)) / 2.0,
        rect.y + rect.h / 2.0,
        glyph,
        color,
        input.metrics,
    ));
    scene.hits.push(Hit { rect: hit, target });
}

fn badges(
    input: &ChromeInput,
    start: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> Option<f32> {
    let scale = input.scale;
    let theme = input.theme;
    let pad = px(BADGE_PAD, scale);
    let mut right = start;
    let mut left = None;
    for badge in right_items(input) {
        let (fill_color, text_color) = match badge.kind {
            BadgeKind::Copy | BadgeKind::Zoom => (
                blend(theme.background, theme.accent, BADGE_TINT),
                theme.accent,
            ),
            BadgeKind::Prefix => (tint(theme, BADGE_NEUTRAL_TINT), theme.foreground),
        };
        let width = (measure(&badge.text, false) * input.ui.small + pad * 2.0)
            .max(px(BADGE_HEIGHT, scale))
            .round();
        let rect = bar_item(right - width, width, BADGE_HEIGHT, scale);
        scene
            .quads
            .push(rounded(rect, fill_color, BADGE_RADIUS, scale));
        scene.text.push(small_text(
            rect.x + pad,
            rect,
            badge.text,
            text_color,
            input.ui,
        ));
        left = Some(rect.x);
        right = rect.x - px(ITEM_GAP, scale);
    }
    left
}

fn right_items<'a>(input: &ChromeInput<'a>) -> impl Iterator<Item = BadgeParts> + 'a {
    let zoom = input
        .layout
        .is_some_and(|layout| layout.zoomed)
        .then(|| BadgeParts {
            kind: BadgeKind::Zoom,
            text: "ZOOM".into(),
        });
    let badges = &input.status.badges;
    let of = move |kind| {
        badges
            .iter()
            .filter(move |badge| badge.kind == kind)
            .cloned()
    };
    of(BadgeKind::Copy).chain(zoom).chain(of(BadgeKind::Prefix))
}

fn tabs(
    input: &ChromeInput,
    start: f32,
    end: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let tabs = &input.status.tabs;
    let pad = px(TAB_PAD, scale);
    let gap = px(TAB_GAP, scale);
    let labels: Vec<String> = tabs
        .iter()
        .map(|tab| {
            let name = ellipsize(&tab.name, px(TAB_NAME_MAX, scale), false, measure);
            format!("{} {name}", tab.index)
        })
        .collect();
    let extras: Vec<f32> =
        tabs.iter()
            .map(|tab| match input.status.tab_icon.and(tab.icon) {
                Some(icon) => icon_lead(Some(icon), input.metrics, scale),
                None => tab_alert(input, tab)
                    .map_or(0.0, |_| px(DOT, scale) + px(PROCESS_ICON_GAP, scale)),
            })
            .collect();
    let widths: Vec<f32> = labels
        .iter()
        .zip(&extras)
        .map(|(label, extra)| (measure(label, false) + extra + pad * 2.0).round())
        .collect();
    let sizes = (widths.as_slice(), extras.as_slice());
    let active = tabs.iter().position(|tab| tab.active).unwrap_or(0);
    let mut placed = place(&labels, sizes, active, (start, end), (pad, gap), measure);
    let mut lead = false;
    if placed.len() < tabs.len() {
        let widest = (measure(&format!("+{}", tabs.len()), false) + pad * 2.0).round() + gap;
        let room = |sides: (bool, bool)| {
            (
                start + if sides.0 { widest } else { 0.0 },
                end - if sides.1 { widest } else { 0.0 },
            )
        };
        let mut sides = hidden(&placed, tabs.len());
        let mut reserved = place(&labels, sizes, active, room(sides), (pad, gap), measure);
        let more = hidden(&reserved, tabs.len());
        if (more.0 && !sides.0) || (more.1 && !sides.1) {
            sides = (sides.0 || more.0, sides.1 || more.1);
            reserved = place(&labels, sizes, active, room(sides), (pad, gap), measure);
        }
        let index_width = format!("{} ", tabs[active].index).len();
        if reserved.iter().any(|item| {
            item.index == active && item.label.trim_end_matches(ELLIPSIS).len() > index_width
        }) {
            placed = reserved;
            lead = sides.0;
        }
    }
    let before = placed.first().map_or(0, |item| item.index);
    let after = placed.last().map_or(0, |item| tabs.len() - item.index - 1);
    let mut x = start;
    if lead && before > 0 {
        x += overflow(input, before, x, scene, measure) + gap;
    }
    for item in &placed {
        let rect = bar_item(x, item.width, TAB_HEIGHT, scale);
        draw_tab(input, &tabs[item.index], rect, &item.label, scene, measure);
        x += item.width + gap;
    }
    if after > 0 {
        let width = (measure(&format!("+{after}"), false) + pad * 2.0).round();
        if x + width <= end {
            overflow(input, after, x, scene, measure);
        }
    }
}

fn hidden(placed: &[Placed], count: usize) -> (bool, bool) {
    match (placed.first(), placed.last()) {
        (Some(first), Some(last)) => (first.index > 0, last.index + 1 < count),
        _ => (false, false),
    }
}

fn overflow(
    input: &ChromeInput,
    count: usize,
    x: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let scale = input.scale;
    let pad = px(TAB_PAD, scale);
    let text = format!("+{count}");
    let width = (measure(&text, false) + pad * 2.0).round();
    let rect = bar_item(x, width, TAB_HEIGHT, scale);
    scene.text.push(ui_text(
        rect.x + pad,
        baseline(rect, input.ui),
        text,
        input.theme.surfaces().muted,
        false,
    ));
    width
}

fn place(
    labels: &[String],
    sizes: (&[f32], &[f32]),
    active: usize,
    room: (f32, f32),
    spacing: (f32, f32),
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> Vec<Placed> {
    let (start, end) = room;
    let (widths, extras) = sizes;
    let (pad, gap) = spacing;
    let mut first = 0;
    while first < active && span(&widths[first..=active], gap) > end - start {
        first += 1;
    }
    let mut placed = Vec::new();
    let mut x = start;
    for (index, (label, width)) in labels.iter().zip(widths).enumerate().skip(first) {
        let (label, width) = if x + width <= end {
            (label.clone(), *width)
        } else if x == start {
            let extra = extras[index];
            let label = ellipsize(label, end - start - pad * 2.0 - extra, false, measure);
            let width = (measure(&label, false) + extra + pad * 2.0).round();
            (label, width)
        } else {
            break;
        };
        if label.is_empty() {
            break;
        }
        placed.push(Placed {
            index,
            label,
            width,
        });
        x += width + gap;
    }
    placed
}

fn draw_tab(
    input: &ChromeInput,
    tab: &TabParts,
    rect: PxRect,
    label: &str,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let hit = full_height(rect, scale);
    let prefix = format!("{} ", tab.index);
    let split = label
        .char_indices()
        .zip(prefix.chars())
        .find(|((_, ours), theirs)| ours != theirs)
        .map_or_else(|| label.len().min(prefix.len()), |((index, _), _)| index);
    let (index, name) = label.split_at(split);
    let hover = hovered(input, hit);
    let pill = match (tab.active, hover) {
        (true, _) => Some(TAB_ACTIVE_TINT),
        (false, true) => Some(TAB_HOVER_TINT),
        (false, false) => None,
    };
    if let Some(percent) = pill {
        scene
            .quads
            .push(rounded(rect, tint(theme, percent), TAB_RADIUS, scale));
    }
    let x = rect.x + px(TAB_PAD, scale);
    let base = baseline(rect, input.ui);
    let index_text = index.trim_end();
    if !index_text.is_empty() {
        scene.text.push(ui_text(
            x,
            base,
            index_text.to_owned(),
            surfaces.faint,
            false,
        ));
    }
    if !name.is_empty() {
        let color = match tab.active || hover {
            true => theme.foreground,
            false => surfaces.muted,
        };
        let start = (x + measure(index, false)).round();
        let icon = input.status.tab_icon.zip(tab.icon);
        let name_x = match icon {
            Some((IconSide::Before, glyph)) => {
                (start + icon_lead(Some(glyph), input.metrics, scale)).round()
            }
            Some((IconSide::After, _)) | None => start,
        };
        scene
            .text
            .push(ui_text(name_x, base, name.to_owned(), color, false));
        let alert = tab_alert(input, tab);
        let centre = cap_centre(rect, input.ui);
        let dot = px(DOT, scale);
        let after = name_x + measure(name, false) + px(PROCESS_ICON_GAP, scale);
        let spot = match icon {
            Some((side, glyph)) => {
                let icon_x = match side {
                    IconSide::Before => start,
                    IconSide::After => after.round(),
                };
                scene.text.push(icon_text(
                    icon_x,
                    centre,
                    glyph,
                    tab_icon_color(input, tab),
                    input.metrics,
                ));
                let width = icon_width(glyph, input.metrics);
                (icon_x + width - dot / 2.0, centre - width / 2.0 - dot / 2.0)
            }
            None => (after, centre - dot / 2.0),
        };
        if let Some(color) = alert {
            let at = PxRect {
                x: spot.0.round(),
                y: spot.1.round(),
                w: dot,
                h: dot,
            };
            scene.quads.push(rounded(at, color, DOT / 2.0, scale));
        }
    }
    scene.hits.push(Hit {
        rect: hit,
        target: HitTarget::Window(tab.window),
    });
}

fn tab_alert(input: &ChromeInput, tab: &TabParts) -> Option<Rgb> {
    tab.agent
        .and_then(|(state, unseen)| agents_layout::alert(input.theme, state, unseen))
}

pub(crate) fn tab_icon_color(input: &ChromeInput, tab: &TabParts) -> Rgb {
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let working = matches!(tab.agent, Some((AgentState::Working, _))).then_some(theme.accent);
    tab_alert(input, tab)
        .or(working)
        .unwrap_or(match tab.active {
            true => surfaces.bright,
            false => surfaces.muted,
        })
}

fn field(
    input: &ChromeInput,
    prompt: BarField,
    start: f32,
    end: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    let line = px(LINE, scale);
    let pad = px(ITEM_PAD, scale);
    let width = (end - start).round();
    if width <= pad * 2.0 + line {
        return;
    }
    let rect = bar_item(start, width, ITEM_HEIGHT, scale);
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: line,
        }),
        ..rounded(rect, tint(input.theme, TAB_HOVER_TINT), FIELD_RADIUS, scale)
    });
    let max = width - pad * 2.0 - line;
    let label = ellipsize(prompt.label, max, false, measure);
    let label_width = measure(&label, false);
    let text_x = rect.x + pad + label_width;
    let max = max - label_width;
    let head: String = prompt.text.chars().take(prompt.cursor).collect();
    let rest: String = prompt.text.chars().skip(prompt.cursor).collect();
    let head = tail(&head, max, false, measure);
    let head_width = measure(&head, false);
    let rest = ellipsize(&rest, max - head_width, false, measure);
    let caret = (text_x + head_width).round();
    let text = format!("{head}{rest}");
    let base = baseline(rect, input.ui);
    if !label.is_empty() {
        scene
            .text
            .push(ui_text(rect.x + pad, base, label, surfaces.muted, false));
    }
    if !text.is_empty() {
        scene.text.push(ui_text(
            text_x.round(),
            base,
            text,
            input.theme.foreground,
            false,
        ));
    }
    scene.quads.push(fill(
        PxRect {
            x: caret,
            y: (rect.y + (rect.h - input.ui.height) / 2.0).round(),
            w: line,
            h: input.ui.height.round(),
        },
        input.theme.accent,
    ));
}

fn hovered(input: &ChromeInput, rect: PxRect) -> bool {
    input.hover.is_some_and(|(x, y)| {
        x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
    })
}

fn full_height(rect: PxRect, scale: f32) -> PxRect {
    PxRect {
        y: 0.0,
        h: bar_height(scale),
        ..rect
    }
}

fn bar_item(x: f32, width: f32, height: f32, scale: f32) -> PxRect {
    let height = px(height, scale);
    PxRect {
        x: x.round(),
        y: ((bar_height(scale) - height) / 2.0).round(),
        w: width,
        h: height,
    }
}

pub(crate) fn baseline(rect: PxRect, ui: UiMetrics) -> f32 {
    (rect.y + (rect.h - ui.height) / 2.0 + ui.ascent).round()
}

pub(crate) fn cap_centre(rect: PxRect, ui: UiMetrics) -> f32 {
    baseline(rect, ui) - ui.cap / ui.small / 2.0
}

pub(crate) fn ui_text(x: f32, baseline: f32, text: String, color: Rgb, bold: bool) -> TextRun {
    TextRun {
        x,
        baseline,
        text,
        color,
        bold,
        face: TextFace::Ui,
    }
}

pub(crate) fn icon_lead(icon: Option<char>, metrics: CellMetrics, scale: f32) -> f32 {
    icon.map_or(0.0, |icon| {
        icon_width(icon, metrics) + px(PROCESS_ICON_GAP, scale)
    })
}

pub(crate) fn icon_text(
    x: f32,
    centre: f32,
    icon: char,
    color: Rgb,
    metrics: CellMetrics,
) -> TextRun {
    TextRun {
        x: x.round(),
        baseline: icon_baseline(centre, metrics),
        text: icon.to_string(),
        color,
        bold: false,
        face: TextFace::PromptIcon,
    }
}

pub(crate) fn small_text(x: f32, rect: PxRect, text: String, color: Rgb, ui: UiMetrics) -> TextRun {
    TextRun {
        x,
        baseline: (rect.y + (rect.h - ui.height * ui.small) / 2.0 + ui.ascent * ui.small).round(),
        text,
        color,
        bold: false,
        face: TextFace::Small,
    }
}

pub(crate) fn ellipsize(
    text: &str,
    max: f32,
    bold: bool,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> String {
    let widths = advances(text, bold, measure);
    if widths.iter().sum::<f32>() <= max {
        return text.to_owned();
    }
    let ellipsis = measure(ELLIPSIS, bold);
    if ellipsis > max {
        return String::new();
    }
    let mut prefix = 0.0;
    let count = widths
        .iter()
        .take_while(|width| {
            prefix += *width;
            prefix + ellipsis <= max
        })
        .count();
    text.chars().take(count).collect::<String>() + ELLIPSIS
}

pub(crate) fn tail(
    text: &str,
    max: f32,
    bold: bool,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> String {
    let widths = advances(text, bold, measure);
    if widths.iter().sum::<f32>() <= max {
        return text.to_owned();
    }
    let ellipsis = measure(ELLIPSIS, bold);
    if ellipsis > max {
        return String::new();
    }
    let mut used = ellipsis;
    let count = widths
        .iter()
        .rev()
        .take_while(|width| {
            used += *width;
            used <= max
        })
        .count();
    ELLIPSIS
        .chars()
        .chain(text.chars().skip(widths.len() - count))
        .collect()
}

fn advances(text: &str, bold: bool, measure: &mut impl FnMut(&str, bool) -> f32) -> Vec<f32> {
    text.chars()
        .map(|c| measure(c.encode_utf8(&mut [0; 4]), bold))
        .collect()
}

fn span(widths: &[f32], gap: f32) -> f32 {
    widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32
}

pub(crate) fn fill(rect: PxRect, color: Rgb) -> Quad {
    Quad {
        rect,
        color: opaque(color),
        radius: 0.0,
        border: None,
    }
}

pub(crate) fn rounded(rect: PxRect, color: Rgb, radius: f32, scale: f32) -> Quad {
    Quad {
        rect,
        color: opaque(color),
        radius: px(radius, scale),
        border: None,
    }
}

pub(crate) fn tint(theme: &Theme, percent: u8) -> Rgb {
    blend(theme.background, theme.foreground, percent)
}

pub(crate) fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

#[cfg(test)]
#[path = "chrome_bar_tests.rs"]
mod tests;
