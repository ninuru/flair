use super::*;
use flair_core::{
    config::Config,
    geom::{CellPoint, GridSize},
    proto::{Cell, Cursor, CursorShape, PaneFlags, PaneUpdate},
};

#[test]
fn compose_resolves_inverse_and_cursor() {
    let theme = Config::default().theme();
    let mut mirror = Mirror::default();
    mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(
            0,
            vec![
                Cell {
                    c: 'a',
                    flags: CellFlags::INVERSE,
                    ..Cell::default()
                },
                Cell::default(),
            ],
        )],
        cursor: Cursor {
            point: CellPoint { col: 1, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
    }]);
    let grid = compose(&mirror, PaneId(1), &theme, true);
    assert_eq!(grid.to_lines(), vec!["a"]);
    assert_eq!(grid.cells[0].fg, theme.background);
    assert_eq!(grid.cells[0].bg, theme.foreground);
    assert_eq!(grid.cursor.unwrap().color, theme.cursor);
    assert!(compose(&mirror, PaneId(1), &theme, false).cursor.is_none());
}
