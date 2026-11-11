use super::*;
use flair_core::{
    ids::PaneId,
    proto::{CellFlags, Color},
};

#[test]
fn snapshots_track_text_attributes_damage_and_cursor() {
    let mut term = Terminal::new(GridSize { cols: 12, rows: 3 }, 100);
    term.advance(b"hi\x1b[31;1m!\x1b[0m");
    let first = term.snapshot(PaneId(1), false).unwrap();
    assert!(first.full);
    assert_eq!(first.rows[0].1[0].c, 'h');
    assert_eq!(first.rows[0].1[2].fg, Color::Indexed(1));
    assert!(first.rows[0].1[2].flags.contains(CellFlags::BOLD));
    assert!(term.snapshot(PaneId(1), false).is_none());
    term.advance(b"\x1b[2;1Hx");
    let changed = term.snapshot(PaneId(1), false).unwrap();
    assert!(!changed.full);
    assert_eq!(changed.rows.len(), 1);
    assert_eq!(changed.rows[0].0, 1);
    term.resize(GridSize { cols: 15, rows: 4 });
    assert!(term.snapshot(PaneId(1), false).unwrap().full);
}

#[test]
fn wide_combining_alt_screen_and_queries() {
    let mut term = Terminal::new(GridSize { cols: 12, rows: 3 }, 100);
    term.advance("界e\u{301}".as_bytes());
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert!(frame.rows[0].1[0].flags.contains(CellFlags::WIDE));
    assert!(frame.rows[0].1[1].flags.contains(CellFlags::WIDE_SPACER));
    assert_eq!(frame.rows[0].1[2].extra.as_deref(), Some("\u{301}"));
    term.advance(b"\x1b[6n\x1b]2;hello\x07\x1b[?1049h\x1b[?25l");
    assert_eq!(term.title(), Some("hello"));
    assert!(
        term.take_events()
            .iter()
            .any(|e| matches!(e, TermEvent::PtyWrite(b) if b.ends_with(b"R")))
    );
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full && frame.flags.alt_screen && !frame.cursor.visible);
}

#[test]
fn osc_cwd_and_prompt_marks_survive_chunk_boundaries() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 3 }, 0);
    term.advance(b"\x1b]7;file://localhost/tmp/a%20");
    term.advance(b"b\x1b\\\x1b]133;D;42\x07");
    assert_eq!(term.cwd(), Some(Path::new("/tmp/a b")));
    assert!(
        term.take_events()
            .contains(&TermEvent::Mark(MarkKind::CommandEnd, Some(42)))
    );
    term.advance(b"\x1b]7;file://foreign.invalid/etc\x07");
    assert_eq!(term.cwd(), Some(Path::new("/tmp/a b")));
}

#[test]
fn shell_facts_surface_as_events_across_advance_calls() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 3 }, 0);
    term.advance(b"\x1b]6973;flair;u=ada;h=b");
    assert!(term.take_events().is_empty());
    term.advance(b"%C3%B6x;v=env\x1b\\");
    assert_eq!(
        term.take_events(),
        vec![TermEvent::Facts(ShellFacts {
            user: "ada".to_string(),
            host: "böx".to_string(),
            venv: Some("env".to_string()),
        })]
    );
}

#[test]
fn scrollback_forces_full_frames_and_returns_to_bottom() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 100);
    term.advance(b"one\r\ntwo\r\nthree");
    term.snapshot(PaneId(1), false).unwrap();
    term.scroll(1);
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full);
    assert_eq!(frame.flags.display_offset, 1);
    assert_eq!(frame.rows[0].1[0].c, 'o');
    term.scroll_to_bottom();
    assert_eq!(
        term.snapshot(PaneId(1), false)
            .unwrap()
            .flags
            .display_offset,
        0
    );
}

#[test]
fn palette_query_uses_configured_background_and_cursor_shape() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 0);
    term.set_palette(&flair_core::config::Config::default().theme());
    term.advance(b"\x1b]11;?\x07\x1b[4 q");
    assert!(term.take_events().iter().any(|e| matches!(e, TermEvent::PtyWrite(bytes) if String::from_utf8_lossy(bytes).contains("0303/0303/0303"))));
    assert_eq!(
        term.snapshot(PaneId(1), false).unwrap().cursor.shape,
        flair_core::proto::CursorShape::Underline
    );
}

#[test]
fn sgr_colors_attributes_and_wrapping_survive_snapshots() {
    let mut term = Terminal::new(GridSize { cols: 8, rows: 3 }, 100);
    term.advance(b"\x1b[91mA\x1b[38;5;123mB\x1b[38;2;12;34;56mC\x1b[0;1;2;3;4;7;8;9mD\x1b[0m12345");
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert_eq!(frame.rows[0].1[0].fg, Color::Indexed(9));
    assert_eq!(frame.rows[0].1[1].fg, Color::Indexed(123));
    assert_eq!(frame.rows[0].1[2].fg, Color::Rgb(12, 34, 56));
    for flag in [
        CellFlags::BOLD,
        CellFlags::DIM,
        CellFlags::ITALIC,
        CellFlags::UNDERLINE,
        CellFlags::INVERSE,
        CellFlags::HIDDEN,
        CellFlags::STRIKE,
    ] {
        assert!(frame.rows[0].1[3].flags.contains(flag));
    }
    assert_eq!(frame.rows[1].1[0].c, '5');
    assert_eq!(
        frame.cursor.point,
        flair_core::geom::CellPoint { col: 1, row: 1 }
    );
}

#[test]
fn cursor_only_updates_and_alt_screen_restore() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 3 }, 100);
    term.advance(b"main");
    term.snapshot(PaneId(1), false).unwrap();
    term.advance(b"\x1b[2;3H\x1b[6 q");
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(!frame.full && frame.rows.is_empty());
    assert_eq!(
        frame.cursor.point,
        flair_core::geom::CellPoint { col: 2, row: 1 }
    );
    assert_eq!(frame.cursor.shape, flair_core::proto::CursorShape::Beam);
    term.advance(b"\x1b[?1049halt");
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full && frame.flags.alt_screen);
    term.advance(b"\x1b[?1049l");
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full && !frame.flags.alt_screen);
    assert_eq!(
        frame.rows[0].1[..4].iter().map(|c| c.c).collect::<String>(),
        "main"
    );
}

#[test]
fn queries_titles_bells_and_modes_are_exposed_to_server() {
    let mut term = Terminal::new(GridSize { cols: 20, rows: 4 }, 0);
    term.advance(b"\x1b[3;7H\x1b[6n\x1b[c\x1b]0;first\x07\x1b]2;last\x1b\\\x07");
    assert_eq!(term.title(), Some("last"));
    let events = term.take_events();
    assert!(events.contains(&TermEvent::PtyWrite(b"\x1b[3;7R".to_vec())));
    assert!(events.contains(&TermEvent::PtyWrite(b"\x1b[?6c".to_vec())));
    assert!(events.contains(&TermEvent::Title("first".into())));
    assert!(events.contains(&TermEvent::Title("last".into())));
    assert!(events.contains(&TermEvent::Bell));
    assert!(term.take_events().is_empty());
    term.advance(b"\x1b[?1h\x1b=\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h\x1b[>1u");
    let modes = term.modes();
    assert!(
        modes.app_cursor
            && modes.app_keypad
            && modes.bracketed_paste
            && modes.focus_events
            && modes.sgr_mouse
    );
    assert_eq!(modes.mouse, MouseMode::Drag);
    assert_eq!(modes.kitty_flags, 1);
}

#[test]
fn drag_selection_has_text_and_viewport_spans_and_clears() {
    let mut term = Terminal::new(GridSize { cols: 12, rows: 3 }, 100);
    term.advance(b"hello world");
    term.select_start(CellPoint { col: 0, row: 0 }, 1);
    assert_eq!(term.selection_text(), None);
    term.select_update(CellPoint { col: 4, row: 0 });
    assert_eq!(term.selection_text().as_deref(), Some("hello"));
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert_eq!(
        frame.selection,
        vec![flair_core::proto::SelectionSpan {
            row: 0,
            start: 0,
            end: 4
        }]
    );
    term.clear_selection();
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.selection.is_empty());
    assert!(frame.rows.is_empty());
    term.select_start(CellPoint { col: 4, row: 0 }, 1);
    term.select_update(CellPoint { col: 0, row: 0 });
    assert_eq!(term.selection_text().as_deref(), Some("hello"));
}

#[test]
fn semantic_and_line_selection_use_terminal_boundaries() {
    let mut term = Terminal::new(GridSize { cols: 20, rows: 3 }, 100);
    term.advance(b"hello world\r\nnext");
    term.select_start(CellPoint { col: 7, row: 0 }, 2);
    assert_eq!(term.selection_text().as_deref(), Some("world"));
    term.select_start(CellPoint { col: 7, row: 0 }, 3);
    assert_eq!(term.selection_text().as_deref(), Some("hello world\n"));
    term.resize(GridSize { cols: 21, rows: 3 });
    assert_eq!(term.selection_text(), None);
    term.select_start(CellPoint { col: 1, row: 0 }, 2);
    term.advance(b"\x1b[2J");
    assert_eq!(term.selection_text(), None);
}

#[test]
fn selection_follows_scrollback_and_includes_wide_combining_text() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 100);
    term.advance("界e\u{301}\r\ntwo\r\nthree".as_bytes());
    term.scroll(1);
    term.select_start(CellPoint { col: 0, row: 0 }, 1);
    term.select_update(CellPoint { col: 2, row: 0 });
    assert_eq!(term.selection_text().as_deref(), Some("界e\u{301}"));
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert_eq!(
        frame.selection,
        vec![flair_core::proto::SelectionSpan {
            row: 0,
            start: 0,
            end: 2
        }]
    );
    term.scroll_to_bottom();
    assert!(term.snapshot(PaneId(1), true).unwrap().selection.is_empty());
}

#[test]
fn drag_selection_keeps_rotated_anchor_when_output_scrolls() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 3 }, 100);
    term.advance(b"alpha\r\nbeta\r\ngamma");
    term.select_start(CellPoint { col: 0, row: 1 }, 1);
    term.select_update(CellPoint { col: 3, row: 1 });
    assert_eq!(term.selection_text().as_deref(), Some("beta"));
    term.advance(b"\r\ndelta");
    assert_eq!(term.selection_text().as_deref(), Some("beta"));
    term.select_update(CellPoint { col: 3, row: 0 });
    assert_eq!(term.selection_text().as_deref(), Some("beta"));
}

fn scrolled_copy_mode() -> Terminal {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 100);
    term.advance(b"one\r\ntwo\r\nthree\r\nfour");
    term.snapshot(PaneId(1), false).unwrap();
    term.enter_copy_mode();
    term.scroll(2);
    term.select_start(CellPoint { col: 0, row: 0 }, 1);
    term.select_update(CellPoint { col: 2, row: 0 });
    assert!(term.copy_mode());
    term
}

fn assert_left_copy_mode(term: &mut Terminal) {
    assert!(!term.copy_mode());
    assert_eq!(term.selection_text(), None);
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full);
    assert!(!frame.flags.copy_mode);
    assert_eq!(frame.flags.display_offset, 0);
}

#[test]
fn copy_mode_puts_the_vi_cursor_in_the_snapshot_and_exit_restores_the_view() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 100);
    term.advance(b"one\r\ntwo\r\nthree\r\nfour");
    term.snapshot(PaneId(1), false).unwrap();
    assert!(!term.copy_mode());
    term.enter_copy_mode();
    assert!(term.copy_mode());
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert!(frame.full && frame.flags.copy_mode);
    assert!(frame.cursor.visible);
    term.scroll(2);
    let frame = term.snapshot(PaneId(1), false).unwrap();
    assert_eq!(frame.flags.display_offset, 2);
    assert_eq!(frame.rows[0].1[0].c, 'o');
    assert!(frame.cursor.visible);
    term.exit_copy_mode();
    assert_left_copy_mode(&mut term);
}

#[test]
fn hidden_terminal_cursor_still_shows_the_vi_cursor() {
    let mut term = Terminal::new(GridSize { cols: 10, rows: 2 }, 100);
    term.advance(b"\x1b[?25l");
    assert!(!term.snapshot(PaneId(1), false).unwrap().cursor.visible);
    term.enter_copy_mode();
    assert!(term.snapshot(PaneId(1), false).unwrap().cursor.visible);
}

#[test]
fn resize_leaves_copy_mode() {
    let mut term = scrolled_copy_mode();
    term.resize(GridSize { cols: 12, rows: 2 });
    assert_left_copy_mode(&mut term);
}

#[test]
fn clear_sequences_leave_copy_mode_even_when_split() {
    for chunks in [
        &[&b"\x1b[2J"[..]][..],
        &[b"\x1b[3J"],
        &[b"\x1bc"],
        &[b"x\x1b", b"[2", b"J"],
        &[b"\x1b[", b"3J"],
        &[b"\x1b", b"c"],
    ] {
        let mut term = scrolled_copy_mode();
        for chunk in chunks {
            term.advance(chunk);
        }
        assert_left_copy_mode(&mut term);
    }
}

#[test]
fn other_output_keeps_copy_mode() {
    let mut term = scrolled_copy_mode();
    term.advance(b"\x1b[2K\x1b[J\x1b[1Jmore\x1b[22J");
    assert!(term.copy_mode());
}

#[test]
fn alt_screen_switch_leaves_copy_mode() {
    let mut term = scrolled_copy_mode();
    term.advance(b"\x1b[?1049h");
    assert!(!term.copy_mode());
    term.enter_copy_mode();
    term.advance(b"\x1b[?1049l");
    assert_left_copy_mode(&mut term);
}

#[test]
fn prompt_anchor_tracks_the_reserved_row_without_leaking_text() {
    let mut term = Terminal::new(GridSize { cols: 12, rows: 5 }, 100);
    term.advance(b"output\r\n\x1b]6973;prompt\x07\r\n> command");
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert_eq!(frame.flags.prompt_row, Some(1));
    assert!(
        frame.rows[1]
            .1
            .iter()
            .all(|cell| cell.c == ' ' && cell.extra.is_none() && cell.flags.is_empty())
    );
    term.select_start(CellPoint { col: 0, row: 1 }, 1);
    term.select_update(CellPoint { col: 8, row: 2 });
    assert_eq!(term.selection_text().as_deref(), Some("> command"));
    term.clear_selection();
    term.advance(b"long enough to wrap around\x08\x08");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(1)
    );
    term.resize(GridSize { cols: 9, rows: 7 });
    let resized = term.snapshot(PaneId(1), true).unwrap();
    let row = resized.flags.prompt_row.expect("prompt survives reflow");
    assert_eq!(resized.rows[usize::from(row + 1)].1[0].c, '>');
    assert!(
        resized.rows[usize::from(row)]
            .1
            .iter()
            .all(|cell| cell.c == ' ')
    );
}

#[test]
fn prompt_anchor_moves_with_scroll_and_never_revives_old_prompts() {
    let mut term = Terminal::new(GridSize { cols: 20, rows: 4 }, 100);
    term.advance(b"\x1b]6973;prompt\x07\r\n> first\r\n\x1b]6973;prompt\x07\r\n> second");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(2)
    );
    term.advance(b"\r\noutput");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(1)
    );
    term.advance(b"\r\noutput\r\noutput");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        None
    );
    term.scroll(4);
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        None
    );
    term.scroll_to_bottom();
    term.advance(b"\x1b]6973;prompt\x07\r\n> third");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(2)
    );
    term.advance(b"\x1b[?1049h");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        None
    );
    term.advance(b"\x1b[?1049l");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(2)
    );
    term.advance(b"\x1b[2J");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        None
    );
}

#[test]
fn prompt_anchor_accepts_split_osc_terminators() {
    let mut term = Terminal::new(GridSize { cols: 12, rows: 4 }, 100);
    term.advance(b"\r\n\x1b]6973;pro");
    term.advance(b"mpt\x1b");
    term.advance(b"\\\r\n> text");
    assert_eq!(
        term.snapshot(PaneId(1), true).unwrap().flags.prompt_row,
        Some(1)
    );
}

#[test]
fn prompt_history_keeps_facts_with_their_grid_rows_through_scroll_and_reflow() {
    let mut term = Terminal::new(GridSize { cols: 24, rows: 6 }, 12);
    term.advance(b"\x1b]6973;prompt\x07\r\nfirst\r\n");
    term.set_prompt_info(flair_core::isles::PromptInfo {
        cwd: Some(PathBuf::from("/first")),
        ..Default::default()
    });
    term.advance(b"\x1b]6973;prompt\x07\r\nsecond");
    term.set_prompt_info(flair_core::isles::PromptInfo {
        cwd: Some(PathBuf::from("/second")),
        ..Default::default()
    });
    let history = term.prompt_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].row, 0);
    assert_eq!(history[1].row, 2);
    assert_ne!(history[0].id, history[1].id);
    assert_eq!(history[0].info.cwd.as_deref(), Some(Path::new("/first")));
    assert_eq!(history[1].info.cwd.as_deref(), Some(Path::new("/second")));
    term.advance(b"\r\nmore\r\nmore\r\nmore");
    assert_eq!(term.prompt_history().len(), 1);
    term.scroll(2);
    assert_eq!(term.prompt_history().len(), 2);
    term.resize(GridSize { cols: 12, rows: 8 });
    let history = term.prompt_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].info.cwd.as_deref(), Some(Path::new("/first")));
    let frame = term.snapshot(PaneId(1), true).unwrap();
    assert!(
        frame
            .rows
            .iter()
            .all(|(_, cells)| cells.iter().all(|cell| cell.extra.is_none()))
    );
    term.select_start(CellPoint { col: 0, row: 0 }, 1);
    term.select_update(CellPoint { col: 11, row: 7 });
    assert!(
        term.selection_text()
            .is_some_and(|text| !text.contains("flair-prompt"))
    );
    term.advance(b"\x1b[?1049h");
    assert!(term.prompt_history().is_empty());
}
