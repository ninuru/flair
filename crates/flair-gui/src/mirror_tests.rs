use super::*;
use flair_core::{
    geom::{CellPoint, CellRect},
    ids::WindowId,
    proto::{Cursor, CursorShape, PaneFlags, PanePlacement, WindowLayout},
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

#[test]
fn layout_keeps_current_panes_and_removes_previous_window_cells() {
    let mut mirror = Mirror::default();
    let first = update(true);
    let second = PaneUpdate {
        pane: PaneId(2),
        ..update(true)
    };
    mirror.apply(&[first, second]);
    let rect = CellRect {
        origin: CellPoint { col: 0, row: 1 },
        size: GridSize { cols: 3, rows: 2 },
    };
    let layout = WindowLayout {
        window: WindowId(2),
        area: rect,
        panes: vec![PanePlacement {
            pane: PaneId(2),
            rect,
            band: 0,
        }],
        separators: vec![],
        active: PaneId(2),
        zoomed: false,
    };
    mirror.retain_layout(&layout);
    assert!(mirror.pane(PaneId(1)).is_none());
    assert!(mirror.pane(PaneId(2)).is_some());
    assert!(!mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        ..update(false)
    }]));
}
