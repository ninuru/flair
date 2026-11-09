use std::{env, fs, path::PathBuf};

use flair_core::{
    proto::{CellFlags, SessionInfo, Target},
    sessionizer::{self, Candidate},
    theme::Theme,
};

use crate::cellgrid::{CellGrid, DrawCell};

pub struct Picker {
    pub query: String,
    entries: Vec<Candidate>,
    selected: usize,
    last: Option<Target>,
}

impl Picker {
    pub fn new(
        sessions: Vec<SessionInfo>,
        current: &str,
        last: Option<Target>,
        directories: Vec<PathBuf>,
    ) -> Self {
        Self {
            query: String::new(),
            entries: sessionizer::candidates(&sessions, current, directories),
            selected: 0,
            last,
        }
    }

    pub fn insert(&mut self, text: &str) {
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, delta: i32) {
        let count = sessionizer::rank(&self.entries, &self.query).len();
        self.selected = self
            .selected
            .saturating_add_signed(delta as isize)
            .min(count.saturating_sub(1));
    }

    pub fn accept(&self) -> Option<Target> {
        if self.query.is_empty()
            && let Some(last) = &self.last
        {
            return Some(last.clone());
        }
        sessionizer::rank(&self.entries, &self.query)
            .get(self.selected)
            .map(|entry| entry.target())
    }

    pub fn draw(&self, grid: &mut CellGrid, theme: &Theme) {
        grid.cursor = None;
        let width = usize::from(grid.size.cols).min(90);
        let entries = sessionizer::rank(&self.entries, &self.query);
        let height = usize::from(grid.size.rows)
            .min(18)
            .min(entries.len().max(1) + 4);
        if width == 0 || height == 0 {
            return;
        }
        let left = (usize::from(grid.size.cols) - width) / 2;
        let top = (usize::from(grid.size.rows) - height) / 2;
        for y in top..top + height {
            for x in left..left + width {
                grid.cells[y * usize::from(grid.size.cols) + x] = DrawCell {
                    c: ' ',
                    extra: None,
                    fg: theme.foreground,
                    bg: theme.background,
                    flags: CellFlags::empty(),
                };
            }
        }
        let write = |grid: &mut CellGrid, row, text: &str, selected| {
            write_row(grid, (left, top + row, width), text, selected, theme);
        };
        write(
            grid,
            0,
            &format!("Sessionizer  {} matches", entries.len()),
            false,
        );
        if height > 1 {
            let query = tail(&self.query, width.saturating_sub(4));
            write(grid, 1, &format!("> {query}_"), false);
        }
        if height < 4 {
            return;
        }
        let capacity = height - 4;
        let first = self.selected.saturating_sub(capacity.saturating_sub(1));
        for (row, (index, entry)) in entries
            .iter()
            .enumerate()
            .skip(first)
            .take(capacity)
            .enumerate()
        {
            let marker = if index == self.selected { ">" } else { " " };
            let open = if entry.open { "*" } else { " " };
            let name_width = (width.saturating_sub(4) / 2).min(24);
            let name: String = entry.name.chars().take(name_width).collect();
            let prefix = format!("{marker}{open} {name:name_width$} ");
            let path = tail(
                &entry.root.to_string_lossy(),
                width.saturating_sub(prefix.chars().count()),
            );
            write(
                grid,
                3 + row,
                &format!("{prefix}{path}"),
                index == self.selected,
            );
        }
        if entries.is_empty() {
            write(
                grid,
                3.min(height - 2),
                "No matches. Edit your search.",
                false,
            );
        }
        let hint = if self.query.is_empty()
            && let Some(Target::Name(name)) = &self.last
        {
            if width >= 27 {
                format!(
                    "Enter last: {}  Esc cancel",
                    tail(name, width.saturating_sub(24))
                )
            } else {
                "Enter last  Esc".into()
            }
        } else if entries.is_empty() {
            "Edit search  Esc cancel".into()
        } else if width >= 38 {
            "Enter switch  Up/Down move  Esc cancel".into()
        } else {
            "Enter switch  Esc cancel".into()
        };
        let hint = if width < 24 {
            if width >= 11 {
                if self.accept().is_some() {
                    "Enter / Esc"
                } else {
                    "Edit / Esc"
                }
            } else {
                "Esc"
            }
        } else {
            &hint
        };
        write(grid, height - 1, hint, false);
    }
}

fn tail(text: &str, width: usize) -> String {
    let chars: Vec<_> = text.chars().filter(|c| !c.is_control()).collect();
    if chars.len() <= width {
        return chars.into_iter().collect();
    }
    if width < 4 {
        return chars
            .into_iter()
            .rev()
            .take(width)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
    format!(
        "...{}",
        chars[chars.len() - (width - 3)..]
            .iter()
            .collect::<String>()
    )
}

fn write_row(
    grid: &mut CellGrid,
    rect: (usize, usize, usize),
    text: &str,
    selected: bool,
    theme: &Theme,
) {
    let (left, row, width) = rect;
    let start = row * usize::from(grid.size.cols) + left;
    let mut chars = text.chars().filter(|c| !c.is_control());
    for cell in &mut grid.cells[start..start + width] {
        cell.c = chars.next().unwrap_or(' ');
        cell.fg = if selected {
            theme.background
        } else {
            theme.foreground
        };
        cell.bg = if selected {
            theme.accent
        } else {
            theme.background
        };
    }
}

pub fn directories() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let roots = [
        home.clone(),
        home.join("projects"),
        home.join("IdeaProjects"),
        home.join("projects/workspaces"),
        home.join("IdeaProjects/workspaces"),
    ];
    roots
        .into_iter()
        .flat_map(|root| match fs::read_dir(&root) {
            Ok(entries) => entries
                .filter_map(|entry| match entry {
                    Ok(entry) => match entry.path().is_dir() {
                        true => Some(entry.path()),
                        false => None,
                    },
                    Err(error) => {
                        log::debug!("could not read sessionizer entry: {error}");
                        None
                    }
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                log::debug!(
                    "could not scan sessionizer root {}: {error}",
                    root.display()
                );
                Vec::new()
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
