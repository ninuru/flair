use flair_core::{
    geom::{CellPoint, CellRect, GridSize},
    key::{Key, KeyAction, KeyInput, NamedKey},
    proto::CellFlags,
    theme::{Rgb, Theme},
};

use crate::{
    CellMetrics,
    cellgrid::{CellGrid, DrawCell},
    ui::{DOCK_WIDTH, PANEL_PAD_X, px},
};

pub struct ThemePicker {
    names: Vec<String>,
    current: String,
    query: String,
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeOutcome {
    Pending,
    Preview(String),
    Cancel,
    Save(String),
}

const HEADER_ROWS: usize = 3;

impl ThemePicker {
    pub fn new(names: Vec<String>, current: &str) -> ThemePicker {
        let selected = names.iter().position(|name| name == current).unwrap_or(0);
        ThemePicker {
            names,
            current: current.to_owned(),
            query: String::new(),
            selected,
        }
    }

    pub fn key(&mut self, key: &KeyInput) -> ThemeOutcome {
        if key.action == KeyAction::Release {
            return ThemeOutcome::Pending;
        }
        let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.super_;
        match key.key {
            Key::Named(NamedKey::Escape) => ThemeOutcome::Cancel,
            Key::Named(NamedKey::Enter) => self.selection().map_or(ThemeOutcome::Pending, |name| {
                ThemeOutcome::Save(name.to_owned())
            }),
            Key::Named(NamedKey::Up) => self.step(-1),
            Key::Named(NamedKey::Down) => self.step(1),
            Key::Char('p' | 'P') if key.mods.ctrl => self.step(-1),
            Key::Char('n' | 'N') if key.mods.ctrl => self.step(1),
            Key::Char('u' | 'U') if key.mods.ctrl => {
                let changed = !self.query.is_empty();
                self.query.clear();
                self.filtered(changed)
            }
            Key::Named(NamedKey::Backspace) => {
                let changed = self.query.pop().is_some();
                self.filtered(changed)
            }
            Key::Char(_) if plain => match &key.text {
                Some(text) => self.insert(text),
                None => ThemeOutcome::Pending,
            },
            Key::Char(_) | Key::Named(_) => ThemeOutcome::Pending,
        }
    }

    pub fn insert(&mut self, text: &str) -> ThemeOutcome {
        let before = self.query.len();
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        self.filtered(self.query.len() != before)
    }

    pub fn refresh(&mut self, current: &str) -> ThemeOutcome {
        current.clone_into(&mut self.current);
        self.preview()
    }

    pub fn matches(&self) -> Vec<&str> {
        let query = self.query.to_lowercase();
        self.names
            .iter()
            .map(String::as_str)
            .filter(|name| name.to_lowercase().contains(&query))
            .collect()
    }

    pub fn area(size: GridSize, metrics: CellMetrics, scale: f32) -> CellRect {
        let margin = px(PANEL_PAD_X, scale);
        let panel = ((px(DOCK_WIDTH, scale) / metrics.width).floor() as u16).min(size.cols);
        let col = ((margin / metrics.width).ceil() as u16).min(panel / 2);
        let row = ((margin / metrics.height).ceil() as u16).min(size.rows / 2);
        let cols = panel - col * 2;
        CellRect {
            origin: CellPoint {
                col: size.cols - col - cols,
                row,
            },
            size: GridSize {
                cols,
                rows: size.rows - row * 2,
            },
        }
    }

    pub fn draw(&self, grid: &mut CellGrid, area: CellRect, theme: &Theme) -> CellRect {
        grid.cursor = None;
        let surfaces = theme.surfaces();
        let left = usize::from(area.origin.col);
        let top = usize::from(area.origin.row);
        let width = usize::from(area.size.cols);
        let height = usize::from(area.size.rows);
        let matches = self.matches();
        let capacity = height.saturating_sub(HEADER_ROWS);
        let first = self.selected.saturating_sub(capacity.saturating_sub(1));
        for row in 0..height {
            let text = match row {
                0 => format!("Themes  {}", matches.len()),
                1 => format!("> {}_", self.query),
                2 => String::new(),
                _ => matches
                    .get(first + row - HEADER_ROWS)
                    .map(|name| self.entry(name, first + row - HEADER_ROWS))
                    .unwrap_or_default(),
            };
            let bg = if row >= HEADER_ROWS
                && first + row - HEADER_ROWS == self.selected
                && !matches.is_empty()
            {
                surfaces.surface3
            } else {
                surfaces.surface1
            };
            write_row(grid, (left, top + row, width), &text, theme.foreground, bg);
        }
        area
    }

    fn entry(&self, name: &str, index: usize) -> String {
        let marker = if index == self.selected { '>' } else { ' ' };
        let saved = if name == self.current { " *" } else { "" };
        format!("{marker} {name}{saved}")
    }

    fn selection(&self) -> Option<&str> {
        self.matches().get(self.selected).copied()
    }

    fn step(&mut self, delta: isize) -> ThemeOutcome {
        let count = self.matches().len();
        let next = self
            .selected
            .saturating_add_signed(delta)
            .min(count.saturating_sub(1));
        if count == 0 || next == self.selected {
            return ThemeOutcome::Pending;
        }
        self.selected = next;
        self.preview()
    }

    fn filtered(&mut self, changed: bool) -> ThemeOutcome {
        if !changed {
            return ThemeOutcome::Pending;
        }
        self.selected = 0;
        self.preview()
    }

    fn preview(&self) -> ThemeOutcome {
        self.selection().map_or(ThemeOutcome::Pending, |name| {
            ThemeOutcome::Preview(name.to_owned())
        })
    }
}

fn write_row(grid: &mut CellGrid, rect: (usize, usize, usize), text: &str, fg: Rgb, bg: Rgb) {
    let (left, row, width) = rect;
    let start = row * usize::from(grid.size.cols) + left;
    let mut chars = text.chars();
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
#[path = "theme_picker_tests.rs"]
mod tests;
