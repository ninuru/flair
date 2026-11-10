use std::{collections::BTreeMap, path::Path, time::Duration};

use flair_core::{
    config::PromptConfig,
    geom::{CellRect, GridSize},
    ids::PaneId,
    isles::{AheadBehind, Changes, Chip, Head, LastCommand, PromptInfo},
    proto::WindowLayout,
};
use unicode_width::UnicodeWidthChar;

use crate::{
    chrome_bar::{Scene, opaque, rounded, small_text, tail},
    chrome_layout::{ChromeInput, cell_rect, chrome_bottom},
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    ui::{CAP_HEIGHT, CAP_PAD, CAP_RADIUS, DOT, ITEM_GAP, LINE, px},
};

const EDGE: f32 = 4.0;
const ICON_GAP: f32 = 4.0;
const COUNT_CAP: u32 = 1000;
const DROP: [Chip; 10] = [
    Chip::Time,
    Chip::Host,
    Chip::User,
    Chip::Kube,
    Chip::Docker,
    Chip::Venv,
    Chip::GitAheadBehind,
    Chip::GitChanges,
    Chip::Exit,
    Chip::GitBranch,
];

#[derive(Clone, Copy)]
pub struct Isles<'a> {
    pub info: &'a BTreeMap<PaneId, PromptInfo>,
    pub config: &'a PromptConfig,
    pub clock: Option<(u8, u8)>,
    pub home: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub chip: Chip,
    pub icon: Option<char>,
    pub text: String,
    pub tone: Tone,
}

pub fn labels(
    info: &PromptInfo,
    config: &PromptConfig,
    clock: Option<(u8, u8)>,
    home: Option<&Path>,
) -> Vec<Label> {
    Chip::ALL
        .into_iter()
        .filter(|chip| config.enabled(*chip))
        .filter_map(|chip| {
            let (text, tone) =
                text(chip, info, config, clock, home).filter(|(text, _)| !text.is_empty())?;
            Some(Label {
                chip,
                icon: icon(chip).filter(|_| config.icons),
                text,
                tone,
            })
        })
        .collect()
}

pub fn line(labels: &[Label]) -> String {
    labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>()
        .join("  ")
}

pub fn fit(widths: &[(Chip, f32)], room: f32, gap: f32) -> Vec<Chip> {
    let kept = |dropped: &[Chip]| -> Vec<(Chip, f32)> {
        widths
            .iter()
            .copied()
            .filter(|(chip, _)| !dropped.contains(chip))
            .collect()
    };
    let span = |chips: &[(Chip, f32)]| {
        chips.iter().map(|(_, width)| width).sum::<f32>()
            + gap * chips.len().saturating_sub(1) as f32
    };
    (0..=DROP.len())
        .map(|count| kept(&DROP[..count]))
        .find(|chips| span(chips) <= room)
        .unwrap_or_else(|| kept(&DROP))
        .into_iter()
        .map(|(chip, _)| chip)
        .collect()
}

pub fn next_minute(since_epoch: Duration) -> Duration {
    let minute = Duration::from_secs(60);
    minute - Duration::from_nanos((since_epoch.as_nanos() % minute.as_nanos()) as u64)
}

pub fn local_clock() -> Option<(u8, u8)> {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let local = unsafe { libc::localtime_r(&now, &mut tm) };
    if local.is_null() {
        return None;
    }
    Some((
        u8::try_from(tm.tm_hour).ok()?,
        u8::try_from(tm.tm_min).ok()?,
    ))
}

fn text(
    chip: Chip,
    info: &PromptInfo,
    config: &PromptConfig,
    clock: Option<(u8, u8)>,
    home: Option<&Path>,
) -> Option<(String, Tone)> {
    let plain = |text: String| Some((text, Tone::Plain));
    let named = |prefix: &str, value: &str| match config.icons {
        true => value.to_owned(),
        false => format!("{prefix} {value}"),
    };
    let shell = info.shell.as_ref();
    let repo = info.git.as_ref();
    match chip {
        Chip::User => plain(shell?.user.clone()),
        Chip::Host => plain(shell?.host.clone()),
        Chip::Cwd => plain(directory(info.cwd.as_deref()?, home)),
        Chip::GitBranch => match &repo?.head {
            Head::Branch(name) | Head::Detached(name) => plain(name.clone()),
        },
        Chip::GitAheadBehind => plain(ahead_behind(repo?.upstream?)?),
        Chip::GitChanges => plain(changes(repo?.changes?)?),
        Chip::Venv => plain(named("venv", shell?.venv.as_deref()?)),
        Chip::Docker => plain(named("docker", info.docker.as_deref()?)),
        Chip::Kube => plain(named("kube", info.kube.as_deref()?)),
        Chip::Exit => exit(info.last?, config.duration_ms),
        Chip::Time => clock.and_then(|(hour, minute)| plain(format!("{hour:02}:{minute:02}"))),
    }
}

fn shortest(text: &str) -> String {
    let last = text.rsplit('/').next().unwrap_or(text);
    let short = format!("…{last}");
    match short.chars().count() < text.chars().count() {
        true => short,
        false => text.to_owned(),
    }
}

fn icon(chip: Chip) -> Option<char> {
    match chip {
        Chip::User => Some('\u{f2c0}'),
        Chip::Host => Some('\u{f109}'),
        Chip::Cwd => Some('\u{f114}'),
        Chip::GitBranch => Some('\u{e725}'),
        Chip::GitChanges => Some('\u{f0f6}'),
        Chip::Venv => Some('\u{e73c}'),
        Chip::Docker => Some('\u{e7b0}'),
        Chip::Kube => Some('\u{f10fe}'),
        Chip::Time => Some('\u{f017}'),
        Chip::GitAheadBehind | Chip::Exit => None,
    }
}

fn directory(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

fn ahead_behind(upstream: AheadBehind) -> Option<String> {
    let capped = |count: u32| match count >= COUNT_CAP {
        true => "999+".to_owned(),
        false => count.to_string(),
    };
    let parts: Vec<String> = [("↑", upstream.ahead), ("↓", upstream.behind)]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(arrow, count)| format!("{arrow}{}", capped(count)))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn changes(changes: Changes) -> Option<String> {
    let lines: Vec<String> = [("+", changes.added), ("−", changes.removed)]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(sign, count)| format!("{sign}{count}"))
        .collect();
    match (changes.files, lines.is_empty()) {
        (0, _) => None,
        (files, true) => Some(files.to_string()),
        (files, false) => Some(format!("{files} · {}", lines.join(" "))),
    }
}

fn exit(last: LastCommand, threshold: u64) -> Option<(String, Tone)> {
    let failed = last.exit.filter(|code| *code != 0);
    let shown = (last.duration.0 >= threshold).then(|| duration(last.duration.0));
    let parts: Vec<String> = failed
        .map(|code| code.to_string())
        .into_iter()
        .chain(shown)
        .collect();
    let tone = match failed {
        Some(_) => Tone::Failed,
        None => Tone::Ok,
    };
    (!parts.is_empty()).then(|| (parts.join(" · "), tone))
}

fn duration(ms: u64) -> String {
    match ms {
        0..1000 => format!("0.{:02}s", ms / 10),
        1000..60_000 => format!("{}.{}s", ms / 1000, ms % 1000 / 100),
        60_000..3_600_000 => format!("{}m {}s", ms / 60_000, ms % 60_000 / 1000),
        _ => format!("{}h {}m", ms / 3_600_000, ms % 3_600_000 / 60_000),
    }
}

pub(crate) fn band(
    input: &ChromeInput,
    window: &WindowLayout,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let Some(isles) = input.isles else {
        return;
    };
    if isles.config.band() == 0 {
        return;
    }
    for placement in &window.panes {
        let Some(info) = isles
            .info
            .get(&placement.pane)
            .filter(|_| placement.band > 0)
        else {
            continue;
        };
        let outer = placement.outer();
        let row = cell_rect(
            input,
            CellRect {
                origin: outer.origin,
                size: GridSize {
                    rows: placement.band,
                    ..outer.size
                },
            },
        );
        let height = px(CAP_HEIGHT, input.scale).min(row.h - px(LINE, input.scale) * 2.0);
        let row = match outer.origin.row {
            0 => {
                let top = chrome_bottom(input.scale);
                PxRect {
                    y: top,
                    h: row.y + row.h - top,
                    ..row
                }
            }
            _ => row,
        };
        let labels = labels(info, isles.config, isles.clock, isles.home);
        chips(input, &labels, row, height, scene, measure);
    }
}

fn chips(
    input: &ChromeInput,
    labels: &[Label],
    row: PxRect,
    height: f32,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    if height <= 0.0 || labels.is_empty() {
        return;
    }
    let gap = px(ITEM_GAP, scale);
    let start = (row.x + px(EDGE, scale)).round();
    let room = row.x + row.w - px(EDGE, scale) - start;
    let widths: Vec<(Chip, f32)> = labels
        .iter()
        .map(|label| {
            let text = match label.chip {
                Chip::Cwd => shortest(&label.text),
                Chip::User
                | Chip::Host
                | Chip::GitBranch
                | Chip::GitAheadBehind
                | Chip::GitChanges
                | Chip::Venv
                | Chip::Docker
                | Chip::Kube
                | Chip::Exit
                | Chip::Time => label.text.clone(),
            };
            (label.chip, width(input, label, &text, measure))
        })
        .collect();
    let kept = fit(&widths, room, gap);
    let others = labels
        .iter()
        .filter(|label| label.chip != Chip::Cwd && kept.contains(&label.chip))
        .map(|label| width(input, label, &label.text, measure))
        .sum::<f32>()
        + gap * kept.len().saturating_sub(1) as f32;
    let mut placed = Vec::new();
    for label in labels.iter().filter(|label| kept.contains(&label.chip)) {
        let text = if label.chip == Chip::Cwd {
            let free = room - others - width(input, label, "", measure);
            let text = tail(&label.text, free / input.ui.small, false, measure);
            if text != label.text && text.chars().count() < 2 {
                continue;
            }
            text
        } else {
            label.text.clone()
        };
        let width = width(input, label, &text, measure);
        placed.push((label, text, width));
    }
    let y = (row.y + (row.h - height) / 2.0).round();
    let mut x = start;
    for (label, text, width) in placed {
        let rect = PxRect {
            x,
            y,
            w: width,
            h: height,
        };
        draw(input, label, text, rect, scene);
        x += width + gap;
    }
}

fn draw(input: &ChromeInput, label: &Label, text: String, rect: PxRect, scene: &mut Scene) {
    let scale = input.scale;
    let surfaces = input.theme.surfaces();
    scene.quads.push(Quad {
        border: Some(Border {
            color: opaque(surfaces.outline),
            width: px(LINE, scale),
        }),
        ..rounded(rect, surfaces.surface2, CAP_RADIUS, scale)
    });
    let x = rect.x + px(CAP_PAD, scale);
    let mark = match label.tone {
        Tone::Plain => None,
        Tone::Ok => Some(surfaces.running),
        Tone::Failed => Some(surfaces.attention),
    };
    match (mark, label.icon) {
        (Some(color), _) => {
            let dot = px(DOT, scale);
            let spot = PxRect {
                x: x.round(),
                y: (rect.y + (rect.h - dot) / 2.0).round(),
                w: dot,
                h: dot,
            };
            scene.quads.push(rounded(spot, color, DOT / 2.0, scale));
        }
        (None, Some(icon)) => scene.text.push(TextRun {
            x: x.round(),
            baseline: (rect.y + rect.h / 2.0 + input.metrics.baseline - input.metrics.height / 2.0)
                .round(),
            text: icon.to_string(),
            color: surfaces.muted,
            bold: false,
            face: TextFace::Mono,
        }),
        (None, None) => {}
    }
    scene.text.push(small_text(
        (x + lead(input, label)).round(),
        rect,
        text,
        surfaces.muted,
        input.ui,
    ));
}

fn width(
    input: &ChromeInput,
    label: &Label,
    text: &str,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> f32 {
    let pad = px(CAP_PAD, input.scale);
    (pad * 2.0 + lead(input, label) + measure(text, false) * input.ui.small).round()
}

fn lead(input: &ChromeInput, label: &Label) -> f32 {
    let scale = input.scale;
    match (label.tone, label.icon) {
        (Tone::Ok | Tone::Failed, _) => px(DOT, scale) + px(ITEM_GAP, scale),
        (Tone::Plain, Some(icon)) => icon_width(input, icon) + px(ICON_GAP, scale),
        (Tone::Plain, None) => 0.0,
    }
}

fn icon_width(input: &ChromeInput, icon: char) -> f32 {
    (icon.width().unwrap_or(1).max(1) as f32 * input.metrics.width).round()
}

#[cfg(test)]
#[path = "isles_tests.rs"]
mod tests;
