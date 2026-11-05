use crate::{
    cellgrid::{CellGrid, DrawCell, DrawCursor},
    mirror::Mirror,
};
use flair_core::{
    geom::GridSize,
    ids::PaneId,
    proto::CellFlags,
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

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
