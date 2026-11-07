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

pub fn compose_status(status: &StatusLine, cols: u16, theme: &Theme) -> StatusRow {
    let width = usize::from(cols);
    let blank = DrawCell {
        c: ' ',
        extra: None,
        fg: theme.foreground,
        bg: Rgb::default(),
        flags: CellFlags::empty(),
    };
    let pending = text_cells(
        status.pending.as_deref().unwrap_or(""),
        width,
        CellFlags::BOLD,
        theme,
    );
    let pending_space = usize::from(!pending.is_empty() && pending.len() < width);
    let remaining = width - pending.len() - pending_space;
    let suffix_width = if status.windows.is_empty() {
        remaining
    } else {
        remaining / 2
    };
    let suffix = session_cells(&status.session, suffix_width, theme);
    let left_width = remaining - suffix.len();
    let mut cells = vec![blank; width];
    let windows = draw_windows(status, &mut cells[..left_width], theme);
    cells[remaining - suffix.len()..remaining].clone_from_slice(&suffix);
    cells[width - pending.len()..].clone_from_slice(&pending);
    StatusRow {
        grid: CellGrid {
            size: GridSize { cols, rows: 1 },
            cells,
            cursor: None,
        },
        windows,
    }
}

fn draw_windows(status: &StatusLine, cells: &mut [DrawCell], theme: &Theme) -> Vec<WindowHit> {
    let entries: Vec<_> = status
        .windows
        .iter()
        .map(|window| {
            let flags = if window.active {
                CellFlags::BOLD | CellFlags::UNDERLINE
            } else {
                CellFlags::empty()
            };
            text_cells(
                &format!(" {}:{}", window.index, window.name),
                cells.len(),
                flags,
                theme,
            )
        })
        .collect();
    let active = status
        .windows
        .iter()
        .position(|window| window.active)
        .unwrap_or(0);
    let mut first = 0;
    let mut used: usize = entries.iter().take(active + 1).map(Vec::len).sum();
    while used > cells.len() && first < active {
        used -= entries[first].len();
        first += 1;
    }
    let mut windows = Vec::new();
    let mut col = 0;
    for (window, entry) in status.windows.iter().zip(entries).skip(first) {
        let mut count = entry.len().min(cells.len() - col);
        if count > 0 && entry[count - 1].flags.contains(CellFlags::WIDE) {
            count -= 1;
        }
        if count < window.index.to_string().len() + 2 {
            break;
        }
        cells[col..col + count].clone_from_slice(&entry[..count]);
        windows.push(WindowHit {
            window: window.id,
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

fn session_cells(session: &str, width: usize, theme: &Theme) -> Vec<DrawCell> {
    if width < 3 {
        return text_cells("| ", width, CellFlags::empty(), theme);
    }
    let mut cells = text_cells("| ", width, CellFlags::empty(), theme);
    cells.extend(text_cells(session, width - 3, CellFlags::empty(), theme));
    cells.extend(text_cells(" ", 1, CellFlags::empty(), theme));
    cells
}

fn text_cells(text: &str, limit: usize, flags: CellFlags, theme: &Theme) -> Vec<DrawCell> {
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
            fg: theme.foreground,
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
                fg: theme.foreground,
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
