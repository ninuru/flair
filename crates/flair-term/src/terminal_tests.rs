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
