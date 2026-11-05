use crate::{
    cellgrid::{CellGrid, DrawCell, DrawCursor},
    mirror::Mirror,
};
use flair_core::{geom::GridSize, ids::PaneId, proto::CellFlags, theme::Theme};

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
        .map(|cell| {
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
