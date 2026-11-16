use super::*;
use flair_core::{
    agent::{AgentKind, AgentState},
    ids::PaneId,
    status::{ActivePane, AgentRow, StatusWindow},
};
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
                program: String::new(),
                active: true,
                last: false,
                activity: false,
                bell: false,
            },
            StatusWindow {
                id: WindowId(2),
                index: 2,
                name: "editor".into(),
                program: String::new(),
                active: false,
                last: true,
                activity: false,
                bell: false,
            },
        ],
        pending: None,
        copy: false,
        pane: None,
        agents: Vec::new(),
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
                program: String::new(),
                active: index == 30,
                last: false,
                activity: false,
                bell: false,
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
            program: String::new(),
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
                icon: None,
                name: "shell".into(),
                active: false,
                last: false,
                activity: false,
                bell: false,
                agent: None,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                icon: None,
                name: "editor".into(),
                active: true,
                last: true,
                activity: false,
                bell: false,
                agent: None,
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

#[test]
fn status_text_keeps_wide_and_combining_characters_whole() {
    let status = StatusLine {
        session: "ü日".into(),
        windows: vec![
            StatusWindow {
                id: WindowId(1),
                index: 1,
                name: "日本語".into(),
                program: String::new(),
                active: false,
                last: false,
                activity: false,
                bell: false,
            },
            StatusWindow {
                id: WindowId(2),
                index: 2,
                name: "e\u{301}dit\u{7}".into(),
                program: String::new(),
                active: true,
                last: false,
                activity: false,
                bell: false,
            },
        ],
        pending: Some("^A".into()),
        ..fixture()
    };
    let lines: Vec<String> = [0, 1, 3, 5, 9, 12, 14, 17, 20, 24, 30, 40]
        .into_iter()
        .map(|cols| line(&status, cols))
        .collect();
    assert_eq!(
        lines,
        [
            "",
            "^",
            " ^A",
            " | ^A",
            " 2:|   ^A",
            " 2:e\u{301}d| ü  ^A",
            " 2:e\u{301}dit| ü  ^A",
            " 2:e\u{301}dit | ü日  ^A",
            " 2:e\u{301}dit    | ü日  ^A",
            " 2:e\u{301}dit        | ü日  ^A",
            " 1:日本語 2:e\u{301}dit     | ü日  ^A",
            " 1:日本語 2:e\u{301}dit               | ü日  ^A",
        ]
    );
}

#[test]
fn status_parts_carry_activity_and_bell() {
    let mut status = fixture();
    status.windows[1].activity = true;
    status.windows[1].bell = true;
    let parts = status_parts(&status);
    assert!(!parts.tabs[0].activity && !parts.tabs[0].bell);
    assert!(parts.tabs[1].activity && parts.tabs[1].bell);
}

#[test]
fn agent_rows_become_parts_with_icon_label_and_the_active_pane() {
    let row = |pane: u32, app: &str, state, kind| AgentRow {
        session: "main".into(),
        window: WindowId(1),
        pane: PaneId(pane),
        app: app.into(),
        state,
        kind,
        title: format!("task {pane}"),
        unseen: pane == 2,
    };
    let status = StatusLine {
        agents: vec![
            row(1, "claude", AgentState::Working, None),
            row(2, "codex", AgentState::Blocked, Some(AgentKind::Permission)),
        ],
        pane: Some(ActivePane {
            pane: PaneId(1),
            cwd: None,
            at_prompt: false,
        }),
        ..fixture()
    };
    let parts = status_parts(&status).agents;
    assert_eq!(
        parts,
        vec![
            AgentParts {
                pane: PaneId(1),
                window: WindowId(1),
                session: "main".into(),
                icon: process_icon("claude"),
                title: "task 1".into(),
                label: "Working",
                state: AgentState::Working,
                unseen: false,
                active: true,
            },
            AgentParts {
                pane: PaneId(2),
                window: WindowId(1),
                session: "main".into(),
                icon: None,
                title: "task 2".into(),
                label: "Needs approval",
                state: AgentState::Blocked,
                unseen: true,
                active: false,
            },
        ]
    );
    assert!(process_icon("claude").is_some());
}

#[test]
fn each_tab_carries_its_most_urgent_agent() {
    let row = |window: u32, pane: u32, state, unseen| AgentRow {
        session: "main".into(),
        window: WindowId(window),
        pane: PaneId(pane),
        app: "claude".into(),
        state,
        kind: None,
        title: String::new(),
        unseen,
    };
    let urgent = |agents: Vec<AgentRow>| {
        let status = StatusLine {
            agents,
            ..fixture()
        };
        status_parts(&status)
            .tabs
            .iter()
            .map(|tab| tab.agent)
            .collect::<Vec<_>>()
    };
    assert_eq!(urgent(Vec::new()), [None, None]);
    assert_eq!(
        urgent(vec![
            row(1, 1, AgentState::Working, false),
            row(1, 2, AgentState::Error, false),
            row(1, 3, AgentState::Blocked, true),
            row(2, 4, AgentState::Idle, false),
        ]),
        [
            Some((AgentState::Error, false)),
            Some((AgentState::Idle, false))
        ]
    );
    assert_eq!(
        urgent(vec![
            row(1, 1, AgentState::Done, true),
            row(1, 2, AgentState::Blocked, false),
            row(2, 3, AgentState::Working, false),
            row(2, 4, AgentState::Done, true),
        ]),
        [
            Some((AgentState::Blocked, false)),
            Some((AgentState::Done, true))
        ]
    );
    assert_eq!(
        urgent(vec![
            row(1, 1, AgentState::Done, false),
            row(1, 2, AgentState::Working, false),
            row(1, 3, AgentState::Idle, false),
        ]),
        [Some((AgentState::Working, false)), None]
    );
}
