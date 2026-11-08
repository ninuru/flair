use super::*;

#[test]
fn rect_contains_its_cells_only() {
    let rect = CellRect {
        origin: CellPoint { col: 2, row: 1 },
        size: GridSize { cols: 3, rows: 2 },
    };
    assert!(rect.contains(CellPoint { col: 2, row: 1 }));
    assert!(rect.contains(CellPoint { col: 4, row: 2 }));
    assert!(!rect.contains(CellPoint { col: 5, row: 1 }));
    assert!(!rect.contains(CellPoint { col: 2, row: 3 }));
    assert!(!rect.contains(CellPoint { col: 1, row: 1 }));
    assert!(!rect.contains(CellPoint { col: 2, row: 0 }));
}

#[test]
fn rect_at_the_grid_edge_does_not_overflow() {
    let rect = CellRect {
        origin: CellPoint {
            col: u16::MAX,
            row: u16::MAX,
        },
        size: GridSize { cols: 1, rows: 1 },
    };
    assert!(rect.contains(CellPoint {
        col: u16::MAX,
        row: u16::MAX,
    }));
}
