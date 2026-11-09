use crate::Terminal;
use flair_core::{
    command::{CopyMotion, SelectionKind},
    geom::{CellPoint, GridSize},
    ids::PaneId,
};

fn term(cols: u16, rows: u16, text: &str) -> Terminal {
    let mut term = Terminal::new(GridSize { cols, rows }, 100);
    term.advance(text.as_bytes());
    term.enter_copy_mode();
    term
}

fn at(col: u16, row: u16) -> CellPoint {
    CellPoint { col, row }
}

fn cursor(term: &mut Terminal) -> CellPoint {
    term.snapshot(PaneId(1), true).unwrap().cursor.point
}

fn offset(term: &mut Terminal) -> u32 {
    term.snapshot(PaneId(1), true).unwrap().flags.display_offset
}

fn moves(term: &mut Terminal, motions: &[CopyMotion]) -> CellPoint {
    for motion in motions {
        term.copy_move(*motion);
    }
    cursor(term)
}

fn numbered(count: usize) -> String {
    (0..count)
        .map(|n| format!("line{n}"))
        .collect::<Vec<_>>()
        .join("\r\n")
}

#[test]
fn cursor_motions_move_the_vi_cursor() {
    let mut term = term(20, 3, "hello world\r\nsecond line");
    assert_eq!(cursor(&mut term), at(11, 1));
    assert_eq!(moves(&mut term, &[CopyMotion::Left]), at(10, 1));
    assert_eq!(moves(&mut term, &[CopyMotion::Up]), at(10, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::Right]), at(11, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::Down]), at(11, 1));
}

#[test]
fn copy_motions_do_nothing_outside_copy_mode() {
    let mut term = term(20, 3, "hello world");
    term.exit_copy_mode();
    term.copy_move(CopyMotion::StartOfLine);
    term.copy_select(SelectionKind::Simple);
    assert_eq!(cursor(&mut term), at(11, 0));
    assert_eq!(term.selection_text(), None);
}

#[test]
fn word_motions_split_semantic_and_whitespace_words() {
    let mut term = term(30, 3, "foo:bar baz.qux end");
    moves(&mut term, &[CopyMotion::StartOfLine]);
    assert_eq!(moves(&mut term, &[CopyMotion::NextWord]), at(3, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::NextWord]), at(4, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::NextWordEnd]), at(6, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::PreviousWord]), at(4, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::NextSpace]), at(8, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::NextSpaceEnd]), at(14, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::PreviousSpace]), at(8, 0));
}

#[test]
fn line_motions_reach_start_indentation_and_end() {
    let mut term = term(20, 3, "   indented text\r\n");
    moves(&mut term, &[CopyMotion::Up]);
    assert_eq!(moves(&mut term, &[CopyMotion::EndOfLine]), at(15, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::BackToIndentation]), at(3, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::StartOfLine]), at(0, 0));
}

#[test]
fn screen_motions_reach_top_middle_and_bottom_lines() {
    let mut term = term(10, 4, &numbered(10));
    assert_eq!(moves(&mut term, &[CopyMotion::TopLine]), at(0, 0));
    assert_eq!(moves(&mut term, &[CopyMotion::MiddleLine]), at(0, 1));
    assert_eq!(moves(&mut term, &[CopyMotion::BottomLine]), at(0, 3));
    assert_eq!(offset(&mut term), 0);
}

#[test]
fn history_top_and_bottom_scroll_and_move_the_cursor() {
    let mut term = term(10, 4, &numbered(10));
    assert_eq!(moves(&mut term, &[CopyMotion::HistoryTop]), at(0, 0));
    assert_eq!(offset(&mut term), 6);
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert_eq!(frame.rows[0].1[4].c, '0');
    assert_eq!(moves(&mut term, &[CopyMotion::HistoryBottom]), at(0, 3));
    assert_eq!(offset(&mut term), 0);
}

#[test]
fn half_pages_scroll_and_keep_the_cursor_row_until_the_edge() {
    let mut term = term(10, 4, &numbered(10));
    moves(&mut term, &[CopyMotion::Up]);
    assert_eq!(moves(&mut term, &[CopyMotion::HalfPageUp]), at(0, 2));
    assert_eq!(offset(&mut term), 2);
    moves(&mut term, &[CopyMotion::HalfPageUp, CopyMotion::HalfPageUp]);
    assert_eq!(offset(&mut term), 6);
    assert_eq!(moves(&mut term, &[CopyMotion::HalfPageUp]), at(0, 0));
    assert_eq!(offset(&mut term), 6);
    assert_eq!(moves(&mut term, &[CopyMotion::HalfPageDown]), at(0, 0));
    assert_eq!(offset(&mut term), 4);
    moves(
        &mut term,
        &[CopyMotion::HalfPageDown, CopyMotion::HalfPageDown],
    );
    assert_eq!(offset(&mut term), 0);
    assert_eq!(moves(&mut term, &[CopyMotion::HalfPageDown]), at(0, 2));
}

#[test]
fn selection_follows_motions_including_jumps() {
    let mut term = term(10, 4, &numbered(10));
    moves(&mut term, &[CopyMotion::StartOfLine]);
    term.copy_select(SelectionKind::Simple);
    moves(&mut term, &[CopyMotion::HalfPageUp]);
    assert_eq!(term.selection_text().as_deref(), Some("line7\nline8\nl"));
    moves(&mut term, &[CopyMotion::HistoryTop]);
    assert!(term.selection_text().unwrap().starts_with("line0\n"));
    moves(&mut term, &[CopyMotion::HistoryBottom]);
    assert_eq!(term.selection_text().as_deref(), Some("l"));
}

#[test]
fn selection_kinds_toggle_convert_and_swap_ends() {
    let mut term = term(20, 3, "alpha beta\r\ngamma delta");
    moves(&mut term, &[CopyMotion::Up, CopyMotion::StartOfLine]);
    term.copy_select(SelectionKind::Simple);
    assert_eq!(term.selection_text().as_deref(), Some("a"));
    moves(&mut term, &[CopyMotion::NextWordEnd]);
    assert_eq!(term.selection_text().as_deref(), Some("alpha"));
    term.copy_select(SelectionKind::Lines);
    assert_eq!(term.selection_text().as_deref(), Some("alpha beta\n"));
    moves(&mut term, &[CopyMotion::Down]);
    assert_eq!(
        term.selection_text().as_deref(),
        Some("alpha beta\ngamma delta\n")
    );
    term.copy_select(SelectionKind::Block);
    assert_eq!(term.selection_text().as_deref(), Some("alpha\ngamma"));
    term.copy_select(SelectionKind::Block);
    assert_eq!(term.selection_text(), None);
    term.copy_select(SelectionKind::Simple);
    moves(&mut term, &[CopyMotion::EndOfLine]);
    assert_eq!(term.selection_text().as_deref(), Some("a delta"));
    term.copy_other_end();
    assert_eq!(cursor(&mut term), at(4, 1));
    assert_eq!(term.selection_text().as_deref(), Some("a delta"));
    moves(&mut term, &[CopyMotion::StartOfLine]);
    assert_eq!(term.selection_text().as_deref(), Some("gamma delta"));
    term.copy_other_end();
    assert_eq!(cursor(&mut term), at(10, 1));
    moves(&mut term, &[CopyMotion::Left]);
    assert_eq!(term.selection_text().as_deref(), Some("gamma delt"));
}

#[test]
fn search_jumps_to_matches_and_repeats_in_both_directions() {
    let mut term = term(20, 4, "one foo\r\ntwo foo\r\nthree");
    assert_eq!(term.copy_search("fo+", false), Ok(()));
    assert_eq!(cursor(&mut term), at(4, 1));
    term.copy_search_again(false);
    assert_eq!(cursor(&mut term), at(4, 0));
    term.copy_search_again(false);
    assert_eq!(cursor(&mut term), at(4, 1));
    term.copy_search_again(true);
    assert_eq!(cursor(&mut term), at(4, 0));
    assert_eq!(term.copy_search("t", true), Ok(()));
    assert_eq!(cursor(&mut term), at(0, 1));
    term.copy_search_again(false);
    assert_eq!(cursor(&mut term), at(0, 2));
    term.copy_search_again(true);
    assert_eq!(cursor(&mut term), at(0, 1));
}

#[test]
fn search_without_match_or_with_invalid_regex_keeps_the_cursor() {
    let mut term = term(20, 4, "one foo\r\ntwo foo\r\nthree");
    term.copy_search("foo", false).unwrap();
    assert_eq!(term.copy_search("zzz", true), Ok(()));
    assert_eq!(cursor(&mut term), at(4, 1));
    assert!(term.copy_search("(", true).is_err());
    assert_eq!(cursor(&mut term), at(4, 1));
    term.copy_search_again(false);
    assert_eq!(cursor(&mut term), at(4, 1));
}

#[test]
fn search_scrolls_into_history() {
    let mut term = term(10, 4, &numbered(10));
    term.copy_search("line1", false).unwrap();
    assert_eq!(offset(&mut term), 5);
    assert_eq!(cursor(&mut term), at(0, 0));
}
