use super::*;
use crate::cellgrid::DrawCell;
use flair_core::{
    geom::GridSize,
    proto::{Cell, CellFlags},
    theme::Rgb,
};

const GITLAB: &str = "https://gitlab.example.com/dp/software/x/-/merge_requests/8";

fn at(row: u16, col: u16) -> CellPoint {
    CellPoint { row, col }
}

fn row(text: &str, cols: usize) -> Vec<Cell> {
    let mut cells: Vec<Cell> = text
        .chars()
        .map(|c| Cell {
            c,
            ..Cell::default()
        })
        .collect();
    cells.resize(cols, Cell::default());
    cells
}

fn wrapped(mut cells: Vec<Cell>) -> Vec<Cell> {
    if let Some(last) = cells.last_mut() {
        last.flags |= CellFlags::WRAP;
    }
    cells
}

fn url(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    find_urls(&chars)
        .into_iter()
        .map(|range| chars[range].iter().collect())
        .collect()
}

fn span(row: u16, cols: std::ops::Range<u16>) -> Vec<CellPoint> {
    cols.map(|col| at(row, col)).collect()
}

#[test]
fn finds_the_gitlab_url_plain_and_inside_quotes() {
    assert_eq!(url(&format!("open {GITLAB} now")), [GITLAB]);
    assert_eq!(url(&format!("echo \"{GITLAB}\"")), [GITLAB]);
    assert_eq!(url(&format!("'{GITLAB}'")), [GITLAB]);
    assert_eq!(url(&format!("<{GITLAB}>")), [GITLAB]);
    assert_eq!(url(&format!("`{GITLAB}`")), [GITLAB]);
}

#[test]
fn finds_http_urls_and_skips_others() {
    assert_eq!(
        url("a http://a.b/c b file:///tmp/x.txt c ftp://x.y"),
        ["http://a.b/c"]
    );
    assert!(url("hello world").is_empty());
    assert!(url("https:// nothing").is_empty());
}

#[test]
fn trims_trailing_punctuation() {
    for text in [
        "see https://a.com/x.",
        "see https://a.com/x,",
        "https://a.com/x;",
        "https://a.com/x:",
        "https://a.com/x!",
        "https://a.com/x?",
        "https://a.com/x?!.",
    ] {
        assert_eq!(url(text), ["https://a.com/x"], "{text}");
    }
    assert_eq!(url("https://a.com/x?q=1."), ["https://a.com/x?q=1"]);
}

#[test]
fn keeps_balanced_brackets_and_trims_unbalanced_ones() {
    assert_eq!(
        url("(https://en.wikipedia.org/wiki/Rust_(language))"),
        ["https://en.wikipedia.org/wiki/Rust_(language)"]
    );
    assert_eq!(url("(see https://a.com/x)."), ["https://a.com/x"]);
    assert_eq!(url("[https://a.com/x]"), ["https://a.com/x"]);
    assert_eq!(url("https://a.com/[1]"), ["https://a.com/[1]"]);
}

#[test]
fn link_at_returns_the_url_and_its_columns() {
    let text = format!("echo \"{GITLAB}\"");
    let rows = vec![row(&text, 80)];
    let len = GITLAB.chars().count() as u16;
    let link = link_at(&rows, at(0, 10)).unwrap();
    assert_eq!(link.url, GITLAB);
    assert_eq!(link.cells, span(0, 6..6 + len));
    assert_eq!(link_at(&rows, at(0, 6)).unwrap().url, GITLAB);
    assert_eq!(link_at(&rows, at(0, 5 + len)).unwrap().url, GITLAB);
}

#[test]
fn a_click_outside_the_span_finds_nothing() {
    let rows = vec![row("go https://a.com/x. done", 30)];
    assert_eq!(link_at(&rows, at(0, 1)), None);
    assert_eq!(link_at(&rows, at(0, 18)), None);
    assert_eq!(link_at(&rows, at(0, 25)), None);
    assert_eq!(link_at(&rows, at(1, 5)), None);
    assert!(link_at(&rows, at(0, 17)).is_some());
    assert_eq!(link_at(&[row("hello world", 20)], at(0, 3)), None);
}

#[test]
fn wide_chars_before_the_url_keep_columns_right() {
    let mut cells = vec![
        Cell {
            c: '\u{754c}',
            flags: CellFlags::WIDE,
            ..Cell::default()
        },
        Cell {
            c: ' ',
            flags: CellFlags::WIDE_SPACER,
            ..Cell::default()
        },
    ];
    cells.extend(row(" https://a.com", 18));
    let rows = vec![cells];
    let link = link_at(&rows, at(0, 5)).unwrap();
    assert_eq!(link.url, "https://a.com");
    assert_eq!(link.cells, span(0, 3..16));
    assert_eq!(link_at(&rows, at(0, 2)), None);
}

#[test]
fn a_url_wrapped_over_two_rows_is_one_link() {
    let rows = vec![
        wrapped(row("$ https://example", 17)),
        row(".com/a/b ok", 17),
        row("https://other.org", 17),
    ];
    let expected: Vec<CellPoint> = span(0, 2..17).into_iter().chain(span(1, 0..8)).collect();
    for point in [at(0, 4), at(1, 3)] {
        let link = link_at(&rows, point).unwrap();
        assert_eq!(link.url, "https://example.com/a/b");
        assert_eq!(link.cells, expected);
    }
    assert_eq!(link_at(&rows, at(1, 10)), None);
    assert_eq!(link_at(&rows, at(2, 3)).unwrap().url, "https://other.org");
}

#[test]
fn rows_without_wrap_are_not_joined() {
    let rows = vec![row("$ https://example", 17), row(".com/a/b ok", 17)];
    assert_eq!(link_at(&rows, at(0, 4)).unwrap().url, "https://example");
    assert_eq!(link_at(&rows, at(1, 3)), None);
}

#[test]
fn underline_marks_the_link_cells_inside_the_pane_rect() {
    let blank = DrawCell {
        c: ' ',
        extra: None,
        fg: Rgb { r: 0, g: 0, b: 0 },
        bg: Rgb { r: 0, g: 0, b: 0 },
        flags: CellFlags::empty(),
    };
    let size = GridSize { cols: 10, rows: 4 };
    let mut grid = CellGrid {
        size,
        cells: vec![blank; 40],
        cursor: None,
    };
    let link = Link {
        url: "https://a".into(),
        cells: vec![at(0, 0), at(0, 1), at(0, 4), at(0, 5), at(1, 0), at(2, 0)],
    };
    let rect = CellRect {
        origin: at(1, 2),
        size: GridSize { cols: 5, rows: 2 },
    };
    underline(&mut grid, &link, rect);
    let marked: Vec<CellPoint> = (0..4)
        .flat_map(|row| (0..10).map(move |col| at(row, col)))
        .filter(|point| grid.at(*point).flags.contains(CellFlags::UNDERLINE))
        .collect();
    assert_eq!(marked, [at(1, 2), at(1, 3), at(1, 6), at(2, 2)]);
}

#[test]
fn hidden_cells_end_the_link() {
    let mut cells = row("https://bank.example.evil.tld/", 40);
    for cell in &mut cells[20..30] {
        cell.flags |= CellFlags::HIDDEN;
    }
    let link = link_at(&[cells], at(0, 3)).expect("link");
    assert_eq!(link.url, "https://bank.example");
}
