use flair_core::theme::{Rgb, Rgba};

use crate::{
    CellMetrics, UiMetrics,
    chrome_layout::{BarField, ChromeInput, Hit, HitTarget, bar_height},
    isles::{icon_baseline, icon_width},
    notice::Notice,
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    status::{BadgeKind, BadgeParts, TabParts},
    ui::{
        CAP_HEIGHT, CAP_PAD, CAP_RADIUS, CRUMB_GAP, DOT, ELLIPSIS, FIELD_RADIUS, INSET, ITEM_GAP,
        ITEM_HEIGHT, ITEM_PAD, LINE, NOTICE_FIELD_MIN, PROCESS_ICON_GAP, SESSION_NAME_MAX,
        TAB_NAME_MAX, UNDERLINE, UNDERLINE_INSET, px,
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
    let surfaces = input.theme.surfaces();
    let width = input.px.0 as f32;
    let bar = bar_height(scale);
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: bar,
        },
        surfaces.surface1,
    ));
    scene.quads.push(fill(
        PxRect {
            x: 0.0,
            y: bar,
            w: width,
            h: px(LINE, scale),
        },
        surfaces.outline,
    ));
    let end = badges(input, scene, measure)
        .map_or(width - px(INSET, scale), |left| left - px(ITEM_GAP, scale));
    let start = session(input, end, scene, measure) + px(ITEM_GAP, scale);
    match (input.prompt, input.notice) {
        (Some(prompt), Some(notice)) => split(input, prompt, notice, (start, end), scene, measure),
        (Some(prompt), None) => field(input, prompt, start, end, scene, measure),
        (None, Some(notice)) => {
            let x = start + px(ITEM_PAD, scale);
            note(input, notice, (x, end), scene, measure);
        }
        (None, None) => tabs(input, start, end, scene, measure),
    }
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
    let left = px(INSET, scale);
    let name_x = left + px(ITEM_PAD, scale);
    let crumb_width = measure(CRUMB_GAP, false);
    let room = (limit - name_x - crumb_width).min(px(SESSION_NAME_MAX, scale));
    let name = ellipsize(&input.status.chip.session, room, true, measure);
    let right = name_x + measure(&name, true);
    let rect = bar_item(
        left,
        (right + px(ITEM_PAD, scale) - left).round(),
        ITEM_HEIGHT,
        scale,
    );
    let base = baseline(rect, input.ui);
    scene
        .text
        .push(ui_text(name_x, base, name, input.theme.foreground, true));
    scene.text.push(ui_text(
        right.round(),
        base,
        CRUMB_GAP.to_owned(),
        input.theme.surfaces().faint,
        false,
    ));
    scene.hits.push(Hit {
        rect: full_height(rect, scale),
        target: HitTarget::Session,
    });
    right.round() + crumb_width
}

fn badges(
    input: &ChromeInput,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> Option<f32> {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    let pad = px(CAP_PAD, scale);
    let mut right = input.px.0 as f32 - px(INSET, scale);
    let mut left = None;
    for badge in right_items(input) {
        let (fill_color, border_color, text_color) = match badge.kind {
            BadgeKind::Copy | BadgeKind::Zoom => (surfaces.accent_soft, theme.accent, theme.accent),
            BadgeKind::Prefix => (surfaces.surface3, surfaces.outline_strong, theme.foreground),
        };
        let width = (measure(&badge.text, false) * input.ui.small + pad * 2.0).round();
        let rect = bar_item(right - width, width, CAP_HEIGHT, scale);
        scene.quads.push(Quad {
            border: Some(Border {
                color: opaque(border_color),
                width: px(LINE, scale),
            }),
            ..rounded(rect, fill_color, CAP_RADIUS, scale)
        });
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
    let pad = px(ITEM_PAD, scale);
    let gap = px(ITEM_GAP, scale);
    let labels: Vec<String> = tabs
        .iter()
        .map(|tab| {
            let name = ellipsize(&tab.name, px(TAB_NAME_MAX, scale), false, measure);
            format!("{} {name}", tab.index)
        })
        .collect();
    let extras: Vec<f32> = tabs
        .iter()
        .map(|tab| icon_lead(tab.icon, input.metrics, scale))
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
        let rect = bar_item(x, item.width, ITEM_HEIGHT, scale);
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
    let pad = px(ITEM_PAD, scale);
    let text = format!("+{count}");
    let width = (measure(&text, false) + pad * 2.0).round();
    let rect = bar_item(x, width, ITEM_HEIGHT, scale);
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
    let x = rect.x + px(ITEM_PAD, scale);
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
        let color = if tab.active {
            theme.foreground
        } else if hovered(input, hit) {
            surfaces.bright
        } else {
            surfaces.muted
        };
        let name_x = (x + measure(index, false)).round();
        if let Some(icon) = tab.icon {
            let quiet = match tab.active {
                true => surfaces.bright,
                false => surfaces.muted,
            };
            scene.text.push(icon_text(
                name_x,
                cap_centre(rect, input.ui),
                icon,
                quiet,
                input.metrics,
            ));
        }
        let name_x = (name_x + icon_lead(tab.icon, input.metrics, scale)).round();
        scene
            .text
            .push(ui_text(name_x, base, name.to_owned(), color, false));
    }
    if tab.active {
        let inset = px(UNDERLINE_INSET, scale);
        let height = px(UNDERLINE, scale);
        scene.quads.push(fill(
            PxRect {
                x: rect.x + inset,
                y: bar_height(scale) - height,
                w: (rect.w - inset * 2.0).max(0.0),
                h: height,
            },
            theme.accent,
        ));
    }
    scene.hits.push(Hit {
        rect: hit,
        target: HitTarget::Window(tab.window),
    });
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
        ..rounded(rect, input.theme.background, FIELD_RADIUS, scale)
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
