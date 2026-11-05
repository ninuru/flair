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
        selection: vec![],
    }]);
    let grid = compose(&mirror, PaneId(1), &theme, true);
    assert_eq!(grid.to_lines(), vec!["a"]);
    assert_eq!(grid.cells[0].fg, theme.background);
    assert_eq!(grid.cells[0].bg, theme.foreground);
    assert_eq!(grid.cursor.unwrap().color, theme.cursor);
    assert!(compose(&mirror, PaneId(1), &theme, false).cursor.is_none());
}

#[test]
fn selection_overlay_blends_the_theme_color_and_clears_on_next_update() {
    use flair_core::proto::SelectionSpan;
    let mut theme = Config::default().theme();
    theme.background = Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    theme.selection = flair_core::theme::Rgba {
        r: 110,
        g: 120,
        b: 130,
        a: 255,
    };
    let mut update = PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(0, vec![Cell::default(); 2])],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 0 },
            shape: CursorShape::Block,
            visible: false,
        },
        flags: PaneFlags::default(),
        selection: vec![SelectionSpan {
            row: 0,
            start: 0,
            end: 0,
        }],
    };
    let mut mirror = Mirror::default();
    mirror.apply(&[update.clone()]);
    let grid = compose(&mirror, PaneId(1), &theme, false);
    assert_eq!(
        grid.cells[0].bg,
        Rgb {
            r: 110,
            g: 120,
            b: 130
        }
    );
    assert_eq!(grid.cells[1].bg, theme.background);
    update.full = false;
    update.rows.clear();
    update.selection.clear();
    mirror.apply(&[update]);
    let grid = compose(&mirror, PaneId(1), &theme, false);
    assert!(grid.cells.iter().all(|cell| cell.bg == theme.background));
}
