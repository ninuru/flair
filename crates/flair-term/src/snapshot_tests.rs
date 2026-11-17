use crate::Terminal;
use flair_core::{geom::GridSize, ids::PaneId, proto::CellFlags};

#[test]
fn wrapped_rows_carry_wrap_on_their_last_cell() {
    let mut term = Terminal::new(GridSize { cols: 8, rows: 4 }, 0);
    term.advance(b"https://example.com\r\nshort");
    let frame = term.snapshot(PaneId(1), true).unwrap();
    let wraps = |row: usize| {
        frame.rows[row]
            .1
            .iter()
            .map(|cell| cell.flags.contains(CellFlags::WRAP))
            .collect::<Vec<_>>()
    };
    let last_only = [false, false, false, false, false, false, false, true];
    assert_eq!(wraps(0), last_only);
    assert_eq!(wraps(1), last_only);
    assert_eq!(wraps(2), [false; 8]);
    assert_eq!(wraps(3), [false; 8]);
}
