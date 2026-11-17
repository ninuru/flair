use std::ops::Range;

use flair_core::{
    geom::{CellPoint, CellRect},
    proto::{Cell, CellFlags},
};

use crate::cellgrid::CellGrid;

const SCHEMES: [&str; 2] = ["https://", "http://"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Link {
    pub url: String,
    pub cells: Vec<CellPoint>,
}

pub(crate) fn find_urls(text: &[char]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let Some(scheme) = SCHEMES.iter().find(|scheme| {
            text[start..]
                .iter()
                .copied()
                .take(scheme.len())
                .eq(scheme.chars())
        }) else {
            start += 1;
            continue;
        };
        let body = start + scheme.len();
        let stop = text[body..]
            .iter()
            .position(|c| !url_char(*c))
            .map_or(text.len(), |len| body + len);
        let end = trim(text, start, stop);
        if end > body {
            found.push(start..end);
            start = stop;
        } else {
            start = body;
        }
    }
    found
}

fn url_char(c: char) -> bool {
    !(c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`'))
}

fn trim(text: &[char], start: usize, stop: usize) -> usize {
    let mut end = stop;
    loop {
        let span = &text[start..end];
        match span.last() {
            Some('.' | ',' | ';' | ':' | '!' | '?') => end -= 1,
            Some(&close @ (')' | ']')) if unbalanced(span, close) => end -= 1,
            _ => return end,
        }
    }
}

fn unbalanced(span: &[char], close: char) -> bool {
    let open = if close == ')' { '(' } else { '[' };
    let count = |wanted: char| span.iter().filter(|c| **c == wanted).count();
    count(close) > count(open)
}

fn wraps(row: &[Cell]) -> bool {
    row.last()
        .is_some_and(|cell| cell.flags.contains(CellFlags::WRAP))
}

pub(crate) fn link_at(rows: &[Vec<Cell>], point: CellPoint) -> Option<Link> {
    let row = usize::from(point.row);
    rows.get(row)?;
    let first = (0..row)
        .rev()
        .take_while(|above| wraps(&rows[*above]))
        .last()
        .unwrap_or(row);
    let last = (row..rows.len())
        .find(|below| !wraps(&rows[*below]))
        .unwrap_or(rows.len() - 1);
    let (text, points): (Vec<char>, Vec<CellPoint>) = (first..=last)
        .flat_map(|index| {
            rows[index]
                .iter()
                .enumerate()
                .filter(|(_, cell)| !cell.flags.contains(CellFlags::WIDE_SPACER))
                .map(move |(col, cell)| {
                    (
                        if cell.flags.contains(CellFlags::HIDDEN) {
                            ' '
                        } else {
                            cell.c
                        },
                        CellPoint {
                            row: index as u16,
                            col: col as u16,
                        },
                    )
                })
        })
        .unzip();
    find_urls(&text)
        .into_iter()
        .map(|range| Link {
            url: text[range.clone()].iter().collect(),
            cells: points[range]
                .iter()
                .flat_map(|cell| {
                    let wide = rows[usize::from(cell.row)][usize::from(cell.col)]
                        .flags
                        .contains(CellFlags::WIDE);
                    let spacer = CellPoint {
                        col: cell.col + 1,
                        ..*cell
                    };
                    std::iter::once(*cell).chain(wide.then_some(spacer))
                })
                .collect(),
        })
        .find(|link| link.cells.contains(&point))
}

pub(crate) fn underline(grid: &mut CellGrid, link: &Link, rect: CellRect) {
    let cols = usize::from(grid.size.cols);
    let rows = usize::from(grid.size.rows);
    for point in &link.cells {
        if point.col >= rect.size.cols || point.row >= rect.size.rows {
            continue;
        }
        let col = usize::from(rect.origin.col) + usize::from(point.col);
        let row = usize::from(rect.origin.row) + usize::from(point.row);
        if col < cols && row < rows {
            grid.cells[row * cols + col]
                .flags
                .insert(CellFlags::UNDERLINE);
        }
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
