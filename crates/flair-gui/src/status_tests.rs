use super::*;
use flair_core::status::StatusWindow;
use unicode_width::UnicodeWidthStr;

fn line(status: &StatusLine, cols: u16) -> String {
    status_text(&status_parts(status), cols)
}

#[test]
fn active_entry_and_prefix_are_per_view() {
    let first = fixture();
    let second = second_view(&first);
    assert!(line(&second, 40).ends_with("^A"));
    assert!(!line(&first, 40).contains("^A"));
}

#[test]
fn narrow_grids_keep_the_text_in_bounds() {
    for status in [fixture(), second_view(&fixture())] {
        for cols in 0..=40 {
            assert_eq!(line(&status, cols).width(), usize::from(cols));
        }
    }
}

fn fixture() -> StatusLine {
    StatusLine {
        session: "main".into(),
        hidden: 0,
        running: false,
        finished: false,
        windows: vec![
            StatusWindow {
                id: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: true,
                last: false,
            },
            StatusWindow {
                id: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: false,
                last: true,
            },
        ],
        pending: None,
        copy: false,
    }
}

fn second_view(status: &StatusLine) -> StatusLine {
    StatusLine {
        windows: status
            .windows
            .iter()
            .map(|window| StatusWindow {
                active: window.id == WindowId(2),
                ..window.clone()
            })
            .collect(),
        pending: Some("^A".into()),
        ..status.clone()
    }
}

#[test]
fn overflow_keeps_the_active_window_visible_and_hides_others() {
    let status = StatusLine {
        windows: (1..=30)
            .map(|index| StatusWindow {
                id: WindowId(u32::from(index)),
                index,
                name: "shell".into(),
                active: index == 30,
                last: false,
            })
            .collect(),
        ..fixture()
    };
    let text = line(&status, 24);
    assert!(text.starts_with(" 30:shell"), "{text:?}");
    assert!(!text.contains(" 1:shell"), "{text:?}");
}

#[test]
fn unicode_names_use_display_cells_and_complete_wide_glyphs() {
    let status = StatusLine {
        session: "会話".into(),
        windows: vec![StatusWindow {
            name: "界e\u{301}🙂long".into(),
            ..fixture().windows[0].clone()
        }],
        ..fixture()
    };
    let text = line(&status, 30);
    assert!(text.contains(" 1:界e\u{301}🙂long"), "{text:?}");
    assert!(text.trim_end().ends_with("| 会話"), "{text:?}");
    for cols in 0..=30 {
        assert_eq!(line(&status, cols).width(), usize::from(cols));
    }
}

#[test]
fn long_session_names_leave_room_for_active_entry_and_prefix() {
    let status = StatusLine {
        session: "long-session".repeat(30),
        ..second_view(&fixture())
    };
    let text = line(&status, 24);
    assert!(text.starts_with(" 2:editor"), "{text:?}");
    assert!(text.ends_with("^A"), "{text:?}");
}

#[test]
fn tiny_grids_prioritize_pending_and_never_show_an_unlabelled_entry() {
    for cols in 0..=5 {
        let text = line(&second_view(&fixture()), cols);
        assert!(!text.contains(':'), "{text:?}");
        if cols >= 2 {
            assert!(text.ends_with("^A"), "{text:?}");
        }
    }
    for cols in 0..=3 {
        assert!(!line(&fixture(), cols).contains(':'));
    }
}

#[test]
fn copy_badge_sits_at_the_right_next_to_the_prefix_badge() {
    let copy = StatusLine {
        copy: true,
        ..fixture()
    };
    let text = line(&copy, 40);
    assert!(text.ends_with("| main  COPY"), "{text:?}");
    let both = StatusLine {
        copy: true,
        ..second_view(&fixture())
    };
    let text = line(&both, 40);
    assert!(text.ends_with(" COPY ^A"), "{text:?}");
    assert!(text.starts_with(" 1:shell 2:editor"));
    assert_eq!(line(&copy, 4), "COPY");
}

#[test]
fn status_parts_split_chip_tabs_and_badges() {
    let status = StatusLine {
        hidden: 2,
        running: true,
        finished: false,
        copy: true,
        ..second_view(&fixture())
    };
    let parts = status_parts(&status);
    assert_eq!(
        parts.chip,
        ChipParts {
            session: "main".into(),
            hidden: 2,
            running: true,
            finished: false,
        }
    );
    assert_eq!(
        parts.tabs,
        [
            TabParts {
                window: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: false,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: true,
            },
        ]
    );
    assert_eq!(
        parts.badges,
        [
            BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            },
            BadgeParts {
                kind: BadgeKind::Prefix,
                text: "^A".into(),
            },
        ]
    );
    assert!(status_parts(&fixture()).badges.is_empty());
}

#[test]
fn status_text_keeps_the_old_row_and_adds_chip_marks() {
    let text = status_text(&status_parts(&fixture()), 40);
    assert_eq!(text, " 1:shell 2:editor                | main ");
    let chip = StatusLine {
        hidden: 2,
        running: true,
        finished: true,
        ..fixture()
    };
    let text = status_text(&status_parts(&chip), 40);
    assert_eq!(text.chars().count(), 40);
    assert!(text.starts_with(" 1:shell 2:editor "), "{text:?}");
    assert!(text.ends_with("| main +2 * ! "), "{text:?}");
    let both = StatusLine {
        copy: true,
        ..second_view(&fixture())
    };
    let text = status_text(&status_parts(&both), 40);
    assert!(text.ends_with(" COPY ^A"), "{text:?}");
    for cols in 0..=40 {
        assert_eq!(
            status_text(&status_parts(&chip), cols).chars().count(),
            usize::from(cols)
        );
    }
}
