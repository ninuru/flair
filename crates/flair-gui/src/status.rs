use std::iter;

use flair_core::{
    geom::{CellPoint, GridSize},
    ids::WindowId,
    proto::CellFlags,
    status::StatusLine,
    theme::{Rgb, Theme},
};
use unicode_width::UnicodeWidthChar;

use crate::cellgrid::{CellGrid, DrawCell};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowHit {
    pub window: WindowId,
    pub start: u16,
    pub end: u16,
}

pub struct StatusRow {
    pub grid: CellGrid,
    pub windows: Vec<WindowHit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusParts {
    pub chip: ChipParts,
    pub tabs: Vec<TabParts>,
    pub badges: Vec<BadgeParts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipParts {
    pub session: String,
    pub hidden: u16,
    pub running: bool,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabParts {
    pub window: WindowId,
    pub index: u16,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Prefix,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadgeParts {
    pub kind: BadgeKind,
    pub text: String,
}

impl StatusRow {
    pub fn hit_test(&self, point: CellPoint) -> Option<WindowId> {
        if point.row != 0 {
            return None;
        }
        self.windows
            .iter()
            .find(|hit| point.col >= hit.start && point.col < hit.end)
            .map(|hit| hit.window)
    }
}

pub fn status_parts(status: &StatusLine) -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: status.session.clone(),
            hidden: status.hidden,
            running: status.running,
            finished: status.finished,
        },
        tabs: status
            .windows
            .iter()
            .map(|window| TabParts {
                window: window.id,
                index: window.index,
                name: window.name.clone(),
                active: window.active,
            })
            .collect(),
        badges: [
            status.copy.then(|| BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            }),
            status.pending.clone().map(|text| BadgeParts {
                kind: BadgeKind::Prefix,
                text,
            }),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

pub fn status_text(parts: &StatusParts, cols: u16) -> String {
    let (cells, _) = status_cells(parts, cols, Rgb::default());
    cells
        .iter()
        .filter(|cell| !cell.flags.contains(CellFlags::WIDE_SPACER))
        .flat_map(|cell| {
            iter::once(cell.c).chain(cell.extra.iter().flat_map(|extra| extra.chars()))
        })
        .collect()
}

pub fn compose_status(status: &StatusLine, cols: u16, theme: &Theme) -> StatusRow {
    let (cells, windows) = status_cells(&status_parts(status), cols, theme.foreground);
    StatusRow {
        grid: CellGrid {
            size: GridSize { cols, rows: 1 },
            cells,
            cursor: None,
        },
        windows,
    }
}

fn status_cells(parts: &StatusParts, cols: u16, fg: Rgb) -> (Vec<DrawCell>, Vec<WindowHit>) {
    let width = usize::from(cols);
    let blank = DrawCell {
        c: ' ',
        extra: None,
        fg,
        bg: Rgb::default(),
        flags: CellFlags::empty(),
    };
    let badges = parts
        .badges
        .iter()
        .map(|badge| badge.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let pending = text_cells(&badges, width, CellFlags::BOLD, fg);
    let pending_space = usize::from(!pending.is_empty() && pending.len() < width);
    let remaining = width - pending.len() - pending_space;
    let suffix_width = if parts.tabs.is_empty() {
        remaining
    } else {
        remaining / 2
    };
    let suffix = session_cells(&chip_label(&parts.chip), suffix_width, fg);
    let left_width = remaining - suffix.len();
    let mut cells = vec![blank; width];
    let windows = draw_windows(&parts.tabs, &mut cells[..left_width], fg);
    cells[remaining - suffix.len()..remaining].clone_from_slice(&suffix);
    cells[width - pending.len()..].clone_from_slice(&pending);
    (cells, windows)
}

fn chip_label(chip: &ChipParts) -> String {
    let session = &chip.session;
    let hidden = if chip.hidden > 0 {
        format!(" +{}", chip.hidden)
    } else {
        String::new()
    };
    let running = if chip.running { " *" } else { "" };
    let finished = if chip.finished { " !" } else { "" };
    format!("{session}{hidden}{running}{finished}")
}

fn draw_windows(tabs: &[TabParts], cells: &mut [DrawCell], fg: Rgb) -> Vec<WindowHit> {
    let entries: Vec<_> = tabs
        .iter()
        .map(|tab| {
            let flags = if tab.active {
                CellFlags::BOLD | CellFlags::UNDERLINE
            } else {
                CellFlags::empty()
            };
            text_cells(
                &format!(" {}:{}", tab.index, tab.name),
                cells.len(),
                flags,
                fg,
            )
        })
        .collect();
    let active = tabs.iter().position(|tab| tab.active).unwrap_or(0);
    let mut first = 0;
    let mut used: usize = entries.iter().take(active + 1).map(Vec::len).sum();
    while used > cells.len() && first < active {
        used -= entries[first].len();
        first += 1;
    }
    let mut windows = Vec::new();
    let mut col = 0;
    for (tab, entry) in tabs.iter().zip(entries).skip(first) {
        let mut count = entry.len().min(cells.len() - col);
        if count > 0 && entry[count - 1].flags.contains(CellFlags::WIDE) {
            count -= 1;
        }
        if count < tab.index.to_string().len() + 2 {
            break;
        }
        cells[col..col + count].clone_from_slice(&entry[..count]);
        windows.push(WindowHit {
            window: tab.window,
            start: col as u16,
            end: (col + count) as u16,
        });
        col += count;
        if col == cells.len() {
            break;
        }
    }
    windows
}

fn session_cells(session: &str, width: usize, fg: Rgb) -> Vec<DrawCell> {
    if width < 3 {
        return text_cells("| ", width, CellFlags::empty(), fg);
    }
    let mut cells = text_cells("| ", width, CellFlags::empty(), fg);
    cells.extend(text_cells(session, width - 3, CellFlags::empty(), fg));
    cells.extend(text_cells(" ", 1, CellFlags::empty(), fg));
    cells
}

fn text_cells(text: &str, limit: usize, flags: CellFlags, fg: Rgb) -> Vec<DrawCell> {
    let mut cells: Vec<DrawCell> = Vec::new();
    let mut base: Option<usize> = None;
    for c in text.chars() {
        let Some(width) = c.width() else {
            continue;
        };
        if width == 0 {
            if let Some(index) = base {
                cells[index].extra.get_or_insert_with(String::new).push(c);
            }
            continue;
        }
        if cells.len() + width > limit {
            break;
        }
        base = Some(cells.len());
        cells.push(DrawCell {
            c,
            extra: None,
            fg,
            bg: Rgb::default(),
            flags: flags
                | if width == 2 {
                    CellFlags::WIDE
                } else {
                    CellFlags::empty()
                },
        });
        if width == 2 {
            cells.push(DrawCell {
                c: ' ',
                extra: None,
                fg,
                bg: Rgb::default(),
                flags: flags | CellFlags::WIDE_SPACER,
            });
        }
    }
    cells
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
