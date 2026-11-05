use super::*;

const FG: Rgb = Rgb {
    r: 0xd4,
    g: 0xd4,
    b: 0xd4,
};
const BG: Rgb = Rgb { r: 3, g: 3, b: 3 };

fn cell(c: char, flags: CellFlags) -> DrawCell {
    DrawCell {
        c,
        extra: None,
        fg: FG,
        bg: BG,
        flags,
    }
}

fn grid(rows: &[&str], cols: u16) -> CellGrid {
    let cells = rows
        .iter()
        .flat_map(|row| {
            let mut chars: Vec<DrawCell> =
                row.chars().map(|c| cell(c, CellFlags::empty())).collect();
            chars.resize(usize::from(cols), cell(' ', CellFlags::empty()));
            chars
        })
        .collect();
    CellGrid {
        size: GridSize {
            cols,
            rows: u16::try_from(rows.len()).unwrap(),
        },
        cells,
        cursor: None,
    }
}

#[test]
fn to_lines_trims_trailing_blanks() {
    let grid = grid(&["ab  c", " x", ""], 6);
    assert_eq!(grid.to_lines(), vec!["ab  c", " x", ""]);
}

#[test]
fn wide_char_takes_two_cells() {
    let grid = CellGrid {
        size: GridSize { cols: 4, rows: 1 },
        cells: vec![
            cell('a', CellFlags::empty()),
            cell('世', CellFlags::WIDE),
            cell(' ', CellFlags::WIDE_SPACER),
            cell('b', CellFlags::empty()),
        ],
        cursor: None,
    };
    assert_eq!(grid.to_lines(), vec!["a世b"]);
    assert_eq!(grid.at(CellPoint { col: 3, row: 0 }).c, 'b');
}

#[test]
fn combining_marks_follow_their_base() {
    let mut grid = grid(&["e"], 2);
    grid.cells[0].extra = Some("\u{301}".to_owned());
    assert_eq!(grid.to_lines(), vec!["e\u{301}"]);
}

#[test]
fn at_indexes_row_major() {
    let grid = grid(&["ab", "cd"], 2);
    assert_eq!(grid.at(CellPoint { col: 0, row: 1 }).c, 'c');
    assert_eq!(grid.at(CellPoint { col: 1, row: 1 }).c, 'd');
}
