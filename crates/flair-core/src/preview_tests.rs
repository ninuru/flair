use super::*;

fn text(s: &str) -> Vec<Cell> {
    s.chars()
        .map(|c| Cell {
            c,
            ..Cell::default()
        })
        .collect()
}

fn run(col: u16, len: u16, fg: Option<Color>, bg: Color) -> Ink {
    Ink { col, len, fg, bg }
}

fn size(cols: u16, rows: u16) -> GridSize {
    GridSize { cols, rows }
}

#[test]
fn adjacent_cells_with_the_same_look_merge_and_blanks_are_dropped() {
    let mut cells = text("ab  cd");
    cells[1].fg = Color::Indexed(1);
    let rows = ink(&[(0, cells)], size(6, 1));
    assert_eq!(
        rows,
        [vec![
            run(0, 1, Some(Color::Foreground), Color::Background),
            run(1, 1, Some(Color::Indexed(1)), Color::Background),
            run(4, 2, Some(Color::Foreground), Color::Background),
        ]]
    );
}

#[test]
fn blank_cells_on_a_coloured_background_are_kept_without_ink() {
    let mut cells = text("x   ");
    for cell in &mut cells[1..3] {
        cell.bg = Color::Indexed(4);
    }
    cells[3].flags = CellFlags::HIDDEN;
    cells[3].c = 'h';
    let rows = ink(&[(0, cells)], size(4, 1));
    assert_eq!(
        rows,
        [vec![
            run(0, 1, Some(Color::Foreground), Color::Background),
            run(1, 2, None, Color::Indexed(4)),
        ]]
    );
}

#[test]
fn inverse_swaps_foreground_and_background() {
    let mut cells = text("a ");
    for cell in &mut cells {
        cell.flags = CellFlags::INVERSE;
        cell.fg = Color::Indexed(2);
    }
    let rows = ink(&[(0, cells)], size(2, 1));
    assert_eq!(
        rows,
        [vec![
            run(0, 1, Some(Color::Background), Color::Indexed(2)),
            run(1, 1, None, Color::Indexed(2)),
        ]]
    );
}

#[test]
fn wide_spacers_extend_the_run_of_their_character() {
    let mut cells = text("x\u{4e16} y");
    cells[1].fg = Color::Indexed(3);
    cells[1].flags = CellFlags::WIDE;
    cells[2].flags = CellFlags::WIDE_SPACER;
    let rows = ink(&[(0, cells)], size(4, 1));
    assert_eq!(
        rows,
        [vec![
            run(0, 1, Some(Color::Foreground), Color::Background),
            run(1, 2, Some(Color::Indexed(3)), Color::Background),
            run(3, 1, Some(Color::Foreground), Color::Background),
        ]]
    );
}

#[test]
fn rows_and_columns_outside_the_size_are_clipped() {
    let rows = ink(
        &[(0, text("abcdef")), (1, text("gh")), (2, text("ij"))],
        size(3, 2),
    );
    assert_eq!(
        rows,
        [
            vec![run(0, 3, Some(Color::Foreground), Color::Background)],
            vec![run(0, 2, Some(Color::Foreground), Color::Background)],
        ]
    );
}

#[test]
fn missing_rows_stay_empty() {
    let rows = ink(&[(1, text("a"))], size(2, 3));
    assert_eq!(
        rows,
        [
            vec![],
            vec![run(0, 1, Some(Color::Foreground), Color::Background)],
            vec![]
        ]
    );
}

#[test]
fn rows_beyond_the_cap_are_dropped() {
    let rows: Vec<_> = (0..200).map(|row| (row, text("x"))).collect();
    let out = ink(&rows, size(1, 200));
    assert_eq!(out.len(), ROWS);
    assert!(out.iter().all(|runs| runs.len() == 1));
}

#[test]
fn runs_beyond_the_cap_are_dropped() {
    let cells = text(&"x ".repeat(100));
    let out = ink(&[(0, cells)], size(200, 1));
    let expected: Vec<_> = (0..RUNS)
        .map(|run| {
            let col = u16::try_from(run * 2).unwrap();
            self::run(col, 1, Some(Color::Foreground), Color::Background)
        })
        .collect();
    assert_eq!(out, [expected]);
}
