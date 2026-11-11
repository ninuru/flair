use std::cell::Cell;

use flair_core::{proto::CellFlags, theme::Rgb};

use super::*;
use crate::{cellgrid::DrawCell, font::Style};

const FG: Rgb = Rgb {
    r: 200,
    g: 200,
    b: 200,
};
const BG: Rgb = Rgb { r: 0, g: 0, b: 0 };

fn cells(text: &str) -> Vec<DrawCell> {
    text.chars()
        .map(|c| DrawCell {
            c,
            extra: None,
            fg: FG,
            bg: BG,
            flags: CellFlags::empty(),
        })
        .collect()
}

fn spans(cells: &[DrawCell], cursor: Option<usize>) -> Vec<(usize, usize)> {
    runs(cells, cursor)
        .into_iter()
        .map(|run| (run.start, run.end))
        .collect()
}

#[test]
fn spaces_split_runs_and_runs_without_punctuation_are_skipped() {
    let row = cells("a -> word != x");
    assert_eq!(spans(&row, None), [(2, 4), (10, 12)]);
}

#[test]
fn a_single_punctuation_cell_is_not_a_run() {
    assert_eq!(spans(&cells("a - b"), None), []);
}

#[test]
fn a_style_change_breaks_the_run() {
    let mut row = cells("=>=>");
    row[2].flags = CellFlags::BOLD;
    row[3].flags = CellFlags::BOLD;
    let runs = runs(&row, None);
    assert_eq!(
        runs,
        [
            Run {
                start: 0,
                end: 2,
                style: Style::Regular
            },
            Run {
                start: 2,
                end: 4,
                style: Style::Bold
            },
        ]
    );
}

#[test]
fn a_foreground_change_breaks_the_run() {
    let mut row = cells("-->");
    row[2].fg = Rgb { r: 255, g: 0, b: 0 };
    assert_eq!(spans(&row, None), [(0, 2)]);
}

#[test]
fn a_background_change_like_a_selection_keeps_the_run() {
    let mut row = cells("x->y");
    row[2].bg = Rgb {
        r: 60,
        g: 60,
        b: 120,
    };
    assert_eq!(spans(&row, None), [(0, 4)]);
}

#[test]
fn hidden_wide_and_spacer_cells_break_the_run() {
    for flag in [CellFlags::HIDDEN, CellFlags::WIDE, CellFlags::WIDE_SPACER] {
        let mut row = cells("->->");
        row[2].flags = flag;
        assert_eq!(spans(&row, None), [(0, 2)], "{flag:?}");
    }
}

#[test]
fn combining_marks_break_the_run() {
    let mut row = cells("->->");
    row[1].extra = Some("\u{301}".to_owned());
    assert_eq!(spans(&row, None), [(2, 4)]);
}

#[test]
fn the_block_cursor_cell_breaks_the_run() {
    let row = cells("===>");
    assert_eq!(spans(&row, Some(1)), [(2, 4)]);
    assert_eq!(spans(&row, Some(3)), [(0, 3)]);
}

#[test]
fn row_maps_shaped_glyphs_onto_their_cells() {
    let mut cache = ShapeCache::default();
    let row = cells("a -> b");
    let shaped = cache.row(&row, None, |text, style| {
        assert_eq!((text, style), ("->", Style::Regular));
        vec![Some(Shaped::Blank), Some(Shaped::Glyph(9))]
    });
    assert_eq!(
        shaped,
        [
            None,
            None,
            Some(Shaped::Blank),
            Some(Shaped::Glyph(9)),
            None,
            None
        ]
    );
}

#[test]
fn the_cache_shapes_each_text_and_style_once() {
    let calls = Cell::new(0);
    let mut cache = ShapeCache::default();
    let shape = |_: &str, _: Style| {
        calls.set(calls.get() + 1);
        vec![None, Some(Shaped::Glyph(1))]
    };
    let row = cells("-> ->");
    let expected = [
        None,
        Some(Shaped::Glyph(1)),
        None,
        None,
        Some(Shaped::Glyph(1)),
    ];
    assert_eq!(cache.row(&row, None, shape), expected);
    assert_eq!(cache.row(&row, None, shape), expected);
    assert_eq!(calls.get(), 1);
    let mut bold = cells("->");
    for cell in &mut bold {
        cell.flags = CellFlags::BOLD;
    }
    cache.row(&bold, None, shape);
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_row_without_runs_never_calls_the_shaper() {
    let mut cache = ShapeCache::default();
    let shaped = cache.row(&cells("plain words"), None, |_, _| {
        panic!("nothing to shape")
    });
    assert!(shaped.iter().all(Option::is_none));
}

#[test]
fn style_follows_bold_and_italic() {
    assert_eq!(style(CellFlags::empty()), Style::Regular);
    assert_eq!(style(CellFlags::BOLD), Style::Bold);
    assert_eq!(style(CellFlags::ITALIC), Style::Italic);
    assert_eq!(
        style(CellFlags::BOLD | CellFlags::ITALIC),
        Style::BoldItalic
    );
}
