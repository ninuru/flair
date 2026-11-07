use crate::{
    cellgrid::{CellGrid, DrawCell, DrawCursor},
    mirror::Mirror,
    status::compose_status,
};
use flair_core::{
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::PaneId,
    proto::{CellFlags, WindowLayout},
    status::StatusLine,
    theme::{Rgb, Theme},
};

pub fn compose(mirror: &Mirror, pane: PaneId, theme: &Theme, cursor_visible: bool) -> CellGrid {
    let Some(pane) = mirror.pane(pane) else {
        return CellGrid {
            size: GridSize { cols: 0, rows: 0 },
            cells: vec![],
            cursor: None,
        };
    };
    let cells = pane
        .rows
        .iter()
        .flatten()
        .enumerate()
        .map(|(index, cell)| {
            let mut fg = theme.resolve(cell.fg);
            let mut bg = theme.resolve(cell.bg);
            if cell.flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.flags.contains(CellFlags::DIM) {
                fg.r = (u16::from(fg.r) * 2 / 3) as u8;
                fg.g = (u16::from(fg.g) * 2 / 3) as u8;
                fg.b = (u16::from(fg.b) * 2 / 3) as u8;
            }
            let row = (index / usize::from(pane.size.cols)) as u16;
            let col = (index % usize::from(pane.size.cols)) as u16;
            if pane
                .selection
                .iter()
                .any(|span| span.row == row && col >= span.start && col <= span.end)
            {
                let selection = theme.selection;
                let blend = |over, under| {
                    ((u32::from(over) * u32::from(selection.a)
                        + u32::from(under) * (255 - u32::from(selection.a)))
                        / 255) as u8
                };
                bg = Rgb {
                    r: blend(selection.r, bg.r),
                    g: blend(selection.g, bg.g),
                    b: blend(selection.b, bg.b),
                };
            }
            DrawCell {
                c: if cell.flags.contains(CellFlags::HIDDEN) {
                    ' '
                } else {
                    cell.c
                },
                extra: cell.extra.clone(),
                fg,
                bg,
                flags: cell.flags,
            }
        })
        .collect();
    let cursor = (cursor_visible
        && pane.cursor.visible
        && pane.cursor.point.col < pane.size.cols
        && pane.cursor.point.row < pane.size.rows)
        .then_some(DrawCursor {
            point: pane.cursor.point,
            shape: pane.cursor.shape,
            color: theme.cursor,
        });
    CellGrid {
        size: pane.size,
        cells,
        cursor,
    }
}

pub fn compose_frame(
    mirror: &Mirror,
    layout: &WindowLayout,
    status: &StatusLine,
    size: GridSize,
    theme: &Theme,
    cursor_visible: bool,
) -> CellGrid {
    let mut grid = CellGrid {
        size,
        cells: vec![
            DrawCell {
                c: ' ',
                extra: None,
                fg: theme.foreground,
                bg: theme.background,
                flags: CellFlags::empty()
            };
            usize::from(size.cols) * usize::from(size.rows)
        ],
        cursor: None,
    };
    if size.rows == 0 || size.cols == 0 {
        return grid;
    }
    for placement in &layout.panes {
        let pane = compose(
            mirror,
            placement.pane,
            theme,
            cursor_visible && placement.pane == layout.active,
        );
        place_pane(&mut grid, &pane, placement.rect);
    }
    for separator in &layout.separators {
        let c = match separator.axis {
            Axis::Horizontal => '│',
            Axis::Vertical => '─',
        };
        let right = usize::from(separator.rect.origin.col) + usize::from(separator.rect.size.cols);
        let bottom = usize::from(separator.rect.origin.row) + usize::from(separator.rect.size.rows);
        for row in usize::from(separator.rect.origin.row).max(1)..bottom.min(usize::from(size.rows))
        {
            for col in usize::from(separator.rect.origin.col)..right.min(usize::from(size.cols)) {
                grid.cells[row * usize::from(size.cols) + col].c = c;
            }
        }
    }
    grid.cells[..usize::from(size.cols)]
        .clone_from_slice(&compose_status(status, size.cols, theme).grid.cells);
    grid
}

fn place_pane(grid: &mut CellGrid, pane: &CellGrid, rect: CellRect) {
    let left = usize::from(rect.origin.col);
    let top = usize::from(rect.origin.row);
    let width = usize::from(pane.size.cols.min(rect.size.cols))
        .min(usize::from(grid.size.cols).saturating_sub(left));
    let height = usize::from(pane.size.rows.min(rect.size.rows))
        .min(usize::from(grid.size.rows).saturating_sub(top));
    for row in 0..height {
        if top + row == 0 {
            continue;
        }
        let source = row * usize::from(pane.size.cols);
        let count = width
            - usize::from(
                width > 0
                    && pane.cells[source + width - 1]
                        .flags
                        .contains(CellFlags::WIDE),
            );
        let target = (top + row) * usize::from(grid.size.cols) + left;
        if count > 0 {
            grid.cells[target..target + count]
                .clone_from_slice(&pane.cells[source..source + count]);
        }
    }
    if let Some(cursor) = pane.cursor
        && usize::from(cursor.point.col) < width
        && usize::from(cursor.point.row) < height
        && top + usize::from(cursor.point.row) > 0
    {
        grid.cursor = Some(DrawCursor {
            point: CellPoint {
                col: (left + usize::from(cursor.point.col)) as u16,
                row: (top + usize::from(cursor.point.row)) as u16,
            },
            ..cursor
        });
    }
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
