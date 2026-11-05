use std::iter;

use flair_core::{
    geom::{CellPoint, GridSize},
    proto::{CellFlags, CursorShape},
    theme::Rgb,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGrid {
    pub size: GridSize,
    pub cells: Vec<DrawCell>,
    pub cursor: Option<DrawCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawCell {
    pub c: char,
    pub extra: Option<String>,
    pub fg: Rgb,
    pub bg: Rgb,
    pub flags: CellFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawCursor {
    pub point: CellPoint,
    pub shape: CursorShape,
    pub color: Rgb,
}

impl CellGrid {
    pub fn at(&self, p: CellPoint) -> &DrawCell {
        &self.cells[usize::from(p.row) * usize::from(self.size.cols) + usize::from(p.col)]
    }

    pub fn to_lines(&self) -> Vec<String> {
        self.cells
            .chunks(usize::from(self.size.cols).max(1))
            .map(|row| {
                let line: String = row
                    .iter()
                    .filter(|cell| !cell.flags.contains(CellFlags::WIDE_SPACER))
                    .flat_map(|cell| {
                        iter::once(cell.c).chain(cell.extra.iter().flat_map(|extra| extra.chars()))
                    })
                    .collect();
                line.trim_end_matches(' ').to_owned()
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "cellgrid_tests.rs"]
mod tests;
