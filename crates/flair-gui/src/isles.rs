use std::{collections::BTreeMap, path::Path, time::Duration};

use flair_core::{
    config::{PromptAppearance, PromptConfig, PromptDateFormat, PromptPath, PromptTimeFormat},
    geom::{CellPoint, CellRect, GridSize},
    ids::PaneId,
    isles::{AheadBehind, Changes, Chip, Head, LastCommand, PromptInfo},
    proto::{PanePlacement, WindowLayout},
    theme::{Rgb, Theme, blend},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    CellMetrics,
    chrome_bar::{Scene, fill, opaque, rounded},
    chrome_layout::{ChromeInput, Hit, HitTarget, cell_rect},
    font::PROMPT_ICON_SCALE,
    mirror::Mirror,
    overlay_layout::{OverlayInput, OverlayScene},
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    ui::{ELLIPSIS, LINE, px},
};

const EDGE: f32 = 0.0;
const PAD: f32 = 6.0;
const GAP: f32 = 8.0;
const RADIUS: f32 = 3.0;
const ICON_GAP: f32 = 4.0;
const COUNT_CAP: u32 = 1000;
const DROP: [Chip; 11] = [
    Chip::Date,
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
    pub mirror: &'a Mirror,
    pub config: &'a PromptConfig,
    pub clock: Option<(u8, u8)>,
    pub date: Option<(i32, u8, u8)>,
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
    labels_at(info, config, clock, local_date(), home)
}

pub fn labels_at(
    info: &PromptInfo,
    config: &PromptConfig,
    clock: Option<(u8, u8)>,
    date: Option<(i32, u8, u8)>,
    home: Option<&Path>,
) -> Vec<Label> {
    config
        .order
        .chips()
        .iter()
        .copied()
        .chain(
            Chip::ALL
                .into_iter()
                .filter(|chip| !config.order.chips().contains(chip)),
        )
        .filter(|chip| config.enabled(*chip))
        .filter_map(|chip| {
            let (text, tone) =
                text(chip, info, config, clock, date, home).filter(|(text, _)| !text.is_empty())?;
            Some(Label {
                chip,
                icon: icon(chip).filter(|_| icons(config, chip)),
                text,
                tone,
            })
        })
        .collect()
}

fn icons(config: &PromptConfig, chip: Chip) -> bool {
    config
        .segments
        .get(&chip)
        .and_then(|segment| segment.icons)
        .unwrap_or(config.icons)
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

#[derive(Debug, Clone, Copy)]
pub struct Calendar {
    pub clock: (u8, u8),
    pub date: (i32, u8, u8),
}

pub fn calendar_at(timestamp: u64) -> Option<Calendar> {
    let time = libc::time_t::try_from(timestamp).ok()?;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return None;
    }
    Some(Calendar {
        clock: (
            u8::try_from(tm.tm_hour).ok()?,
            u8::try_from(tm.tm_min).ok()?,
        ),
        date: (
            tm.tm_year + 1900,
            u8::try_from(tm.tm_mon + 1).ok()?,
            u8::try_from(tm.tm_mday).ok()?,
        ),
    })
}

fn local_calendar() -> Option<Calendar> {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    calendar_at(u64::try_from(now).ok()?)
}

pub fn local_clock() -> Option<(u8, u8)> {
    Some(local_calendar()?.clock)
}

pub fn local_date() -> Option<(i32, u8, u8)> {
    Some(local_calendar()?.date)
}

fn text(
    chip: Chip,
    info: &PromptInfo,
    config: &PromptConfig,
    clock: Option<(u8, u8)>,
    date: Option<(i32, u8, u8)>,
    home: Option<&Path>,
) -> Option<(String, Tone)> {
    let plain = |text: String| {
        Some((
            text.chars().filter(|c| !c.is_control()).collect(),
            Tone::Plain,
        ))
    };
    let named = |chip, prefix: &str, value: &str| match icons(config, chip) {
        true => value.to_owned(),
        false => format!("{prefix} {value}"),
    };
    let shell = info.shell.as_ref();
    let repo = info.git.as_ref();
    match chip {
        Chip::User => plain(shell?.user.clone()),
        Chip::Host => plain(shell?.host.clone()),
        Chip::Cwd => plain(directory(info.cwd.as_deref()?, config.path, home)),
        Chip::GitBranch => match &repo?.head {
            Head::Branch(name) | Head::Detached(name) => plain(name.clone()),
        },
        Chip::GitAheadBehind => plain(ahead_behind(repo?.upstream?)?),
        Chip::GitChanges => plain(changes(repo?.changes?)?),
        Chip::Venv => plain(named(chip, "venv", shell?.venv.as_deref()?)),
        Chip::Docker => plain(named(chip, "docker", info.docker.as_deref()?)),
        Chip::Kube => plain(named(chip, "kube", info.kube.as_deref()?)),
        Chip::Exit => exit(info.last?, config.duration_ms),
        Chip::Time => clock.and_then(|(hour, minute)| {
            plain(match config.time_format {
                PromptTimeFormat::TwentyFour => format!("{hour:02}:{minute:02}"),
                PromptTimeFormat::Twelve => format!(
                    "{}:{minute:02} {}",
                    match hour % 12 {
                        0 => 12,
                        hour => hour,
                    },
                    if hour < 12 { "AM" } else { "PM" }
                ),
            })
        }),
        Chip::Date => date.and_then(|date| plain(date_text(date, config.date_format))),
    }
}

fn shortest(text: &str) -> String {
    let short = format!("{ELLIPSIS}/{}", text.rsplit('/').next().unwrap_or(text));
    match short.chars().count() < text.chars().count() {
        true => short,
        false => text.to_owned(),
    }
}

fn cut(text: &str, mut fits: impl FnMut(&str) -> bool) -> String {
    let folders: Vec<&str> = text.split('/').collect();
    let last: Vec<char> = folders
        .last()
        .copied()
        .unwrap_or_default()
        .chars()
        .collect();
    std::iter::once(text.to_owned())
        .chain((1..folders.len()).map(|start| format!("{ELLIPSIS}/{}", folders[start..].join("/"))))
        .chain((0..last.len()).map(|skip| {
            ELLIPSIS
                .chars()
                .chain(last[skip..].iter().copied())
                .collect()
        }))
        .find(|candidate| fits(candidate))
        .unwrap_or_default()
}

fn icon(chip: Chip) -> Option<char> {
    match chip {
        Chip::User => Some('\u{f415}'),
        Chip::Host => Some('\u{ea7a}'),
        Chip::Cwd => Some('\u{f114}'),
        Chip::GitBranch => Some('\u{f418}'),
        Chip::GitChanges => Some('\u{f440}'),
        Chip::Venv => Some('\u{f487}'),
        Chip::Docker => Some('\u{f308}'),
        Chip::Kube => Some('\u{f10fe}'),
        Chip::Time => Some('\u{f017}'),
        Chip::Date => Some('\u{f073}'),
        Chip::GitAheadBehind | Chip::Exit => None,
    }
}

fn directory(path: &Path, style: PromptPath, home: Option<&Path>) -> String {
    match style {
        PromptPath::Full => return path.display().to_string(),
        PromptPath::Folder => {
            return path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
        }
        PromptPath::Abbreviated => {}
    }
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

fn date_text((year, month, day): (i32, u8, u8), style: PromptDateFormat) -> String {
    match style {
        PromptDateFormat::Iso => format!("{year:04}-{month:02}-{day:02}"),
        PromptDateFormat::Short => format!("{month:02}/{day:02}/{year:04}"),
        PromptDateFormat::Long => {
            let months = [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ];
            let month = months
                .get(usize::from(month.saturating_sub(1)))
                .copied()
                .unwrap_or_default();
            format!("{month} {day}, {year}")
        }
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

pub(crate) fn band(input: &ChromeInput, window: &WindowLayout, scene: &mut Scene) {
    let Some(isles) = input.isles else { return };
    if isles.config.band() == 0 {
        return;
    }
    for placement in &window.panes {
        if let Some(history) = isles.mirror.history(placement.pane) {
            for snapshot in history {
                let calendar = calendar_at(snapshot.timestamp);
                draw_prompt(
                    input,
                    placement,
                    snapshot.row,
                    &snapshot.info,
                    calendar,
                    scene,
                );
            }
            continue;
        }
        let Some(info) = isles.info.get(&placement.pane) else {
            continue;
        };
        let Some(pane) = isles.mirror.pane(placement.pane) else {
            continue;
        };
        let Some(prompt_row) = pane.flags.prompt_row.filter(|row| {
            *row < placement.rect.size.rows
                && !pane.flags.alt_screen
                && pane.flags.display_offset == 0
        }) else {
            continue;
        };
        let calendar = isles
            .clock
            .zip(isles.date)
            .map(|(clock, date)| Calendar { clock, date });
        draw_prompt(input, placement, prompt_row, info, calendar, scene);
    }
}

fn draw_prompt(
    input: &ChromeInput,
    placement: &PanePlacement,
    row: u16,
    info: &PromptInfo,
    calendar: Option<Calendar>,
    scene: &mut Scene,
) {
    if row >= placement.rect.size.rows {
        return;
    }
    let Some(isles) = input.isles else { return };
    let rect = cell_rect(
        input,
        CellRect {
            origin: CellPoint {
                row: placement.rect.origin.row + row,
                ..placement.rect.origin
            },
            size: GridSize {
                rows: 1,
                ..placement.rect.size
            },
        },
    );
    let clock = calendar.map(|calendar| calendar.clock).or(isles.clock);
    let date = calendar.map(|calendar| calendar.date).or(isles.date);
    let labels = labels_at(info, isles.config, clock, date, isles.home);
    let paint = Paint {
        theme: input.theme,
        scale: input.scale,
        metrics: input.metrics,
        config: isles.config,
    };
    if row > 0
        && isles.mirror.pane(placement.pane).is_some_and(|pane| {
            pane.rows.get(usize::from(row - 1)).is_some_and(|cells| {
                cells
                    .iter()
                    .all(|cell| cell.c == ' ' && cell.extra.is_none())
            })
        })
    {
        scene
            .quads
            .push(divider(rect, input.metrics, input.scale, input.theme));
    }
    let hits = paint.chips(&labels, rect, scene);
    scene.hits.extend(hits.into_iter().map(|(chip, rect)| Hit {
        rect,
        target: HitTarget::Prompt {
            pane: placement.pane,
            row,
            chip,
        },
    }));
}

fn divider(rect: PxRect, metrics: CellMetrics, scale: f32, theme: &Theme) -> Quad {
    fill(
        PxRect {
            x: rect.x,
            y: (rect.y - metrics.height / 2.0).round(),
            w: rect.w,
            h: px(LINE, scale),
        },
        blend(theme.background, theme.foreground, 12),
    )
}

pub fn preview(
    input: &OverlayInput,
    metrics: CellMetrics,
    config: &PromptConfig,
    info: &PromptInfo,
    rect: PxRect,
    scene: &mut OverlayScene,
) -> Vec<(Chip, PxRect)> {
    if config.band() == 0 {
        return Vec::new();
    }
    let paint = Paint {
        theme: input.theme,
        scale: input.scale,
        metrics,
        config,
    };
    let labels = labels_at(
        info,
        config,
        Some((15, 48)),
        Some((2026, 11, 10)),
        Some(Path::new("/home/developer")),
    );
    let mut chips = Scene::default();
    let hits = paint.chips(&labels, rect, &mut chips);
    scene.quads.extend(chips.quads);
    scene.text.extend(chips.text);
    hits
}

struct Paint<'a> {
    theme: &'a Theme,
    scale: f32,
    metrics: CellMetrics,
    config: &'a PromptConfig,
}

impl Paint<'_> {
    fn width(&self, label: &Label, text: &str) -> f32 {
        (self.pad(label) * 2.0 + self.lead(label) + text.width() as f32 * self.metrics.width)
            .round()
    }

    fn pad(&self, label: &Label) -> f32 {
        px(
            if label.icon.is_some() && label.chip != Chip::Cwd {
                4.0
            } else {
                PAD
            },
            self.scale,
        )
    }

    fn lead(&self, label: &Label) -> f32 {
        label.icon.map_or(0.0, |icon| {
            (icon_cells(icon) as f32 * self.metrics.width * PROMPT_ICON_SCALE).round()
                + px(ICON_GAP, self.scale)
        })
    }

    fn chips(&self, labels: &[Label], row: PxRect, scene: &mut Scene) -> Vec<(Chip, PxRect)> {
        let height = self.metrics.height.min(row.h);
        if height <= 0.0 || labels.is_empty() {
            return Vec::new();
        }
        let gap = px(GAP, self.scale);
        let start = (row.x + px(EDGE, self.scale)).round();
        let room = row.x + row.w - px(EDGE, self.scale) - start;
        let widths: Vec<_> = labels
            .iter()
            .map(|label| {
                let text = if label.chip == Chip::Cwd {
                    shortest(&label.text)
                } else {
                    label.text.clone()
                };
                (label.chip, self.width(label, &text))
            })
            .collect();
        let kept = fit(&widths, room, gap);
        let others = labels
            .iter()
            .filter(|label| label.chip != Chip::Cwd && kept.contains(&label.chip))
            .map(|label| self.width(label, &label.text))
            .sum::<f32>()
            + gap * kept.len().saturating_sub(1) as f32;
        let y = (row.y + (row.h - height) / 2.0).round();
        let mut x = start;
        let mut hits = Vec::new();
        for label in labels.iter().filter(|label| kept.contains(&label.chip)) {
            let text = if label.chip == Chip::Cwd {
                let free = room - others - self.width(label, "");
                cut(&label.text, |text| {
                    text.width() as f32 * self.metrics.width <= free
                })
            } else {
                label.text.clone()
            };
            if text.is_empty() {
                continue;
            }
            let width = self.width(label, &text);
            let rect = PxRect {
                x,
                y,
                w: width,
                h: height,
            };
            self.draw(label, text, rect, scene);
            hits.push((label.chip, rect));
            x += width + gap;
        }
        hits
    }

    fn draw(&self, label: &Label, text: String, rect: PxRect, scene: &mut Scene) {
        let (fill, ink, border) = colours(self.theme, self.config, label);
        if let Some(border) = border {
            scene.quads.push(Quad {
                border: Some(Border {
                    color: opaque(border),
                    width: px(LINE, self.scale),
                }),
                ..rounded(rect, fill, RADIUS, self.scale)
            });
        }
        let x = (rect.x + self.pad(label)).round();
        let baseline =
            (rect.y + (rect.h - self.metrics.height) / 2.0 + self.metrics.baseline).round();
        if let Some(icon) = label.icon {
            scene.text.push(TextRun {
                x,
                baseline: (rect.y
                    + (rect.h - self.metrics.height * PROMPT_ICON_SCALE) / 2.0
                    + self.metrics.baseline * PROMPT_ICON_SCALE)
                    .round(),
                text: icon.to_string(),
                color: ink,
                bold: false,
                face: TextFace::PromptIcon,
            });
        }
        scene.text.push(TextRun {
            x: (x + self.lead(label)).round(),
            baseline,
            text,
            color: ink,
            bold: true,
            face: TextFace::Mono,
        });
    }
}

fn colours(theme: &Theme, config: &PromptConfig, label: &Label) -> (Rgb, Rgb, Option<Rgb>) {
    let segment = config.segments.get(&label.chip);
    let index = segment.and_then(|segment| segment.colour).map_or_else(
        || match label.chip {
            Chip::Cwd | Chip::Host | Chip::Time | Chip::Date => 6,
            Chip::Docker | Chip::GitChanges | Chip::GitAheadBehind | Chip::Venv => 2,
            Chip::GitBranch | Chip::Kube => 5,
            Chip::User => 3,
            Chip::Exit => match label.tone {
                Tone::Failed => 1,
                Tone::Ok | Tone::Plain => 2,
            },
        },
        |colour| colour.index(),
    );
    let hue = if segment.and_then(|segment| segment.colour).is_some() {
        theme.normal[index]
    } else {
        match label.chip {
            Chip::Cwd => theme.bright[6],
            Chip::Docker => blend(theme.bright[2], theme.bright[6], 50),
            Chip::User
            | Chip::Host
            | Chip::GitBranch
            | Chip::GitAheadBehind
            | Chip::GitChanges
            | Chip::Venv
            | Chip::Kube
            | Chip::Exit
            | Chip::Time
            | Chip::Date => theme.normal[index],
        }
    };
    let appearance = segment
        .and_then(|segment| segment.appearance)
        .unwrap_or(config.appearance);
    let fill = match appearance {
        PromptAppearance::Outlined => blend(theme.background, theme.foreground, 4),
        PromptAppearance::Filled => hue,
        PromptAppearance::Plain => theme.background,
    };
    let ink = match appearance {
        PromptAppearance::Filled => {
            if contrast(fill, theme.background) >= contrast(fill, theme.foreground) {
                theme.background
            } else {
                theme.foreground
            }
        }
        PromptAppearance::Outlined | PromptAppearance::Plain => {
            [hue, theme.bright[index], theme.foreground]
                .into_iter()
                .find(|colour| contrast(fill, *colour) >= 4.5)
                .unwrap_or(theme.foreground)
        }
    };
    let border = match appearance {
        PromptAppearance::Outlined => Some(blend(theme.background, theme.foreground, 12)),
        PromptAppearance::Filled => Some(hue),
        PromptAppearance::Plain => None,
    };
    (fill, ink, border)
}

fn contrast(a: Rgb, b: Rgb) -> f32 {
    let luminance = |color: Rgb| {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        channel(color.r) * 0.2126 + channel(color.g) * 0.7152 + channel(color.b) * 0.0722
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn icon_cells(icon: char) -> usize {
    icon.width().unwrap_or(1).max(1)
}

#[cfg(test)]
#[path = "isles_tests.rs"]
mod tests;
