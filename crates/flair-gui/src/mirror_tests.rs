use super::*;
use flair_core::{
    geom::CellPoint,
    proto::{Cursor, CursorShape, PaneFlags},
};

fn update(full: bool) -> PaneUpdate {
    PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 3, rows: 2 },
        full,
        rows: vec![(
            0,
            vec![
                Cell {
                    c: 'x',
                    ..Cell::default()
                };
                3
            ],
        )],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }
}

#[test]
fn partial_unknown_panes_are_ignored() {
    let mut mirror = Mirror::default();
    assert!(!mirror.apply(&[update(false)]));
    assert!(mirror.pane(PaneId(1)).is_none());
}

#[test]
fn full_then_partial_preserves_untouched_rows() {
    let mut mirror = Mirror::default();
    assert!(mirror.apply(&[update(true)]));
    let mut partial = update(false);
    partial.rows = vec![
        (
            1,
            vec![
                Cell {
                    c: 'y',
                    ..Cell::default()
                };
                3
            ],
        ),
        (99, vec![]),
    ];
    mirror.apply(&[partial]);
    let pane = mirror.pane(PaneId(1)).unwrap();
    assert_eq!(pane.rows[0][0].c, 'x');
    assert_eq!(pane.rows[1][0].c, 'y');
}
