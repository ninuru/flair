use std::collections::HashMap;

use flair_core::proto::CellFlags;

use crate::{cellgrid::DrawCell, font::Style};

const CACHE_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shaped {
    Glyph(u16),
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

pub fn style(flags: CellFlags) -> Style {
    match (
        flags.contains(CellFlags::BOLD),
        flags.contains(CellFlags::ITALIC),
    ) {
        (false, false) => Style::Regular,
        (true, false) => Style::Bold,
        (false, true) => Style::Italic,
        (true, true) => Style::BoldItalic,
    }
}

fn shapeable(cell: &DrawCell) -> bool {
    cell.c != ' '
        && cell.extra.is_none()
        && [CellFlags::HIDDEN, CellFlags::WIDE, CellFlags::WIDE_SPACER]
            .into_iter()
            .all(|flag| !cell.flags.contains(flag))
}

fn push(out: &mut Vec<Run>, cells: &[DrawCell], start: usize, end: usize) {
    let run = &cells[start..end];
    if run.len() > 1 && run.iter().any(|cell| cell.c.is_ascii_punctuation()) {
        out.push(Run {
            start,
            end,
            style: style(cells[start].flags),
        });
    }
}

pub fn runs(cells: &[DrawCell], cursor: Option<usize>) -> Vec<Run> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, cell) in cells.iter().enumerate() {
        let usable = shapeable(cell) && cursor != Some(i);
        if let Some(first) = start {
            let joins =
                cells[first].fg == cell.fg && style(cells[first].flags) == style(cell.flags);
            if !(usable && joins) {
                push(&mut out, cells, first, i);
                start = None;
            }
        }
        if usable && start.is_none() {
            start = Some(i);
        }
    }
    if let Some(first) = start {
        push(&mut out, cells, first, cells.len());
    }
    out
}

#[derive(Default)]
pub struct ShapeCache {
    entries: HashMap<Style, HashMap<String, Vec<Option<Shaped>>>>,
    len: usize,
    text: String,
}

impl ShapeCache {
    pub fn row(
        &mut self,
        cells: &[DrawCell],
        cursor: Option<usize>,
        mut shape: impl FnMut(&str, Style) -> Vec<Option<Shaped>>,
    ) -> Vec<Option<Shaped>> {
        let mut out = vec![None; cells.len()];
        for run in runs(cells, cursor) {
            self.text.clear();
            self.text
                .extend(cells[run.start..run.end].iter().map(|cell| cell.c));
            if !self
                .entries
                .get(&run.style)
                .is_some_and(|entries| entries.contains_key(&self.text))
            {
                if self.len >= CACHE_LIMIT {
                    self.entries.clear();
                    self.len = 0;
                }
                let glyphs = shape(&self.text, run.style);
                self.entries
                    .entry(run.style)
                    .or_default()
                    .insert(self.text.clone(), glyphs);
                self.len += 1;
            }
            let glyphs = &self.entries[&run.style][&self.text];
            for (slot, glyph) in out[run.start..run.end].iter_mut().zip(glyphs) {
                *slot = *glyph;
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
