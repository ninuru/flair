use flair_core::{
    command::{Command, CopyCommand, WindowRef},
    geom::CellRect,
    key::{Key, KeyAction, KeyInput, NamedKey},
    proto::CellFlags,
    status::{StatusLine, StatusWindow},
    theme::{Rgb, Theme},
};

use crate::{
    cellgrid::{CellGrid, DrawCell},
    picker::centred,
};

const NAME_LIMIT: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    Choose {
        windows: Vec<StatusWindow>,
        selected: usize,
    },
    Rename {
        text: String,
    },
    Search {
        text: String,
        forward: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Close,
    Run(Command),
}

impl Prompt {
    pub fn choose(status: &StatusLine) -> Prompt {
        Prompt::Choose {
            windows: status.windows.clone(),
            selected: status
                .windows
                .iter()
                .position(|window| window.active)
                .unwrap_or(0),
        }
    }

    pub fn rename(current: String) -> Prompt {
        Prompt::Rename { text: current }
    }

    pub fn search(forward: bool) -> Prompt {
        Prompt::Search {
            text: String::new(),
            forward,
        }
    }

    pub fn insert(&mut self, input: &str) {
        match self {
            Prompt::Choose { .. } => {}
            Prompt::Rename { text } | Prompt::Search { text, .. } => push_name(text, input),
        }
    }

    pub fn key(&mut self, key: &KeyInput) -> Outcome {
        if key.action == KeyAction::Release {
            return Outcome::Pending;
        }
        let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.super_;
        match self {
            Prompt::Choose { windows, selected } => match key.key {
                Key::Named(NamedKey::Escape) | Key::Char('q') if plain => Outcome::Close,
                Key::Named(NamedKey::Enter) => {
                    windows.get(*selected).map_or(Outcome::Close, |window| {
                        Outcome::Run(Command::SelectWindow(WindowRef::Id(window.id)))
                    })
                }
                Key::Named(NamedKey::Up) | Key::Char('k') if plain => {
                    *selected = selected.saturating_sub(1);
                    Outcome::Pending
                }
                Key::Named(NamedKey::Down) | Key::Char('j') if plain => {
                    *selected = (*selected + 1).min(windows.len().saturating_sub(1));
                    Outcome::Pending
                }
                Key::Char(c @ '1'..='9') if plain => Outcome::Run(Command::SelectWindow(
                    WindowRef::Index(c.to_digit(10).map_or(1, |n| n as u16)),
                )),
                Key::Char(_) | Key::Named(_) => Outcome::Pending,
            },
            Prompt::Rename { text } if key.key == Key::Named(NamedKey::Enter) => {
                Outcome::Run(Command::RenameWindow(Some(text.clone())))
            }
            Prompt::Search { text, forward } if key.key == Key::Named(NamedKey::Enter) => {
                Outcome::Run(Command::Copy(CopyCommand::Search {
                    pattern: text.clone(),
                    forward: *forward,
                }))
            }
            Prompt::Rename { text } | Prompt::Search { text, .. } => match key.key {
                Key::Named(NamedKey::Escape) => Outcome::Close,
                Key::Named(NamedKey::Backspace) => {
                    text.pop();
                    Outcome::Pending
                }
                Key::Char('u' | 'U') if key.mods.ctrl => {
                    text.clear();
                    Outcome::Pending
                }
                Key::Char(_) if plain => {
                    if let Some(input) = &key.text {
                        push_name(text, input);
                    }
                    Outcome::Pending
                }
                Key::Char(_) | Key::Named(_) => Outcome::Pending,
            },
        }
    }

    pub fn draw(&self, grid: &mut CellGrid, area: CellRect, theme: &Theme) -> Option<CellRect> {
        grid.cursor = None;
        match self {
            Prompt::Choose { windows, selected } => {
                let width = usize::from(area.size.cols).min(60);
                let height = usize::from(area.size.rows).min(windows.len() + 2);
                let rect = centred(area, width, height);
                let surfaces = theme.surfaces();
                let left = usize::from(rect.origin.col);
                let top = usize::from(rect.origin.row);
                let write = |grid: &mut CellGrid, row, text: &str, fg, bg| {
                    write_row(grid, (left, top + row, width), text, fg, bg);
                };
                for row in 0..height {
                    write(grid, row, "", theme.foreground, surfaces.surface1);
                }
                if height == 0 {
                    return Some(rect);
                }
                write(grid, 0, "Windows", theme.foreground, surfaces.surface1);
                let capacity = height.saturating_sub(2);
                let first = selected.saturating_sub(capacity.saturating_sub(1));
                for (row, (index, window)) in windows
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(capacity)
                    .enumerate()
                {
                    let text = format!(" {}: {}", window.index, window.name);
                    let bg = if index == *selected {
                        surfaces.surface3
                    } else {
                        surfaces.surface1
                    };
                    write(grid, row + 1, &text, theme.foreground, bg);
                }
                if height > 1 {
                    write(
                        grid,
                        height - 1,
                        "Enter select  Esc cancel",
                        surfaces.muted,
                        surfaces.surface1,
                    );
                }
                Some(rect)
            }
            Prompt::Rename { .. } | Prompt::Search { .. } => None,
        }
    }

    pub fn bar_text(&self) -> Option<String> {
        match self {
            Prompt::Choose { .. } => None,
            Prompt::Rename { text } => Some(format!("(rename-window) {text}")),
            Prompt::Search { text, forward } => {
                let label = if *forward { '/' } else { '?' };
                Some(format!("{label}{text}"))
            }
        }
    }
}

fn push_name(text: &mut String, input: &str) {
    for c in input.chars().filter(|c| !c.is_control()) {
        if text.len() + c.len_utf8() > NAME_LIMIT {
            return;
        }
        text.push(c);
    }
}

fn write_row(grid: &mut CellGrid, rect: (usize, usize, usize), text: &str, fg: Rgb, bg: Rgb) {
    let (left, row, width) = rect;
    let start = row * usize::from(grid.size.cols) + left;
    let mut chars = text.chars().filter(|c| !c.is_control());
    for cell in &mut grid.cells[start..start + width] {
        *cell = DrawCell {
            c: chars.next().unwrap_or(' '),
            extra: None,
            fg,
            bg,
            flags: CellFlags::empty(),
        };
    }
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
