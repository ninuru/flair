use crate::{
    geom::GridSize,
    proto::{Cell, CellFlags, Color, Ink},
};

const ROWS: usize = 120;
const RUNS: usize = 64;

pub fn ink(rows: &[(u16, Vec<Cell>)], size: GridSize) -> Vec<Vec<Ink>> {
    let mut out = vec![Vec::new(); usize::from(size.rows).min(ROWS)];
    for (row, cells) in rows {
        if let Some(runs) = out.get_mut(usize::from(*row)) {
            *runs = line(cells, size.cols);
        }
    }
    out
}

fn line(cells: &[Cell], cols: u16) -> Vec<Ink> {
    let mut runs: Vec<Ink> = Vec::new();
    for (col, cell) in (0..cols).zip(cells) {
        let (fg, bg) = look(cell);
        match runs.last_mut() {
            Some(last)
                if cell.flags.contains(CellFlags::WIDE_SPACER)
                    || (last.fg == fg && last.bg == bg) =>
            {
                last.len += 1;
            }
            Some(_) | None => runs.push(Ink {
                col,
                len: 1,
                fg,
                bg,
            }),
        }
    }
    runs.retain(|run| run.fg.is_some() || run.bg != Color::Background);
    runs.truncate(RUNS);
    runs
}

fn look(cell: &Cell) -> (Option<Color>, Color) {
    let (fg, bg) = match cell.flags.contains(CellFlags::INVERSE) {
        true => (cell.bg, cell.fg),
        false => (cell.fg, cell.bg),
    };
    let blank = cell.c == '\0' || cell.c.is_whitespace() || cell.flags.contains(CellFlags::HIDDEN);
    ((!blank).then_some(fg), bg)
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
