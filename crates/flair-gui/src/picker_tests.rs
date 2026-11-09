use super::*;
use flair_core::{config::Config, geom::GridSize, proto::SessionInfo};

fn picker() -> Picker {
    Picker::new(
        vec![SessionInfo {
            name: "flair".into(),
            root: "/projects/flair".into(),
            windows: 1,
        }],
        "main",
        Some(Target::Name("previous".into())),
        Vec::new(),
    )
}

fn grid(cols: u16, rows: u16) -> CellGrid {
    let theme = Config::default().theme();
    CellGrid {
        size: GridSize { cols, rows },
        cells: vec![
            DrawCell {
                c: ' ',
                extra: None,
                fg: theme.foreground,
                bg: theme.background,
                flags: CellFlags::empty()
            };
            usize::from(cols) * usize::from(rows)
        ],
        cursor: None,
    }
}

#[test]
fn empty_enter_uses_last_session_and_query_selects_match() {
    let mut picker = picker();
    assert_eq!(picker.accept(), Some(Target::Name("previous".into())));
    picker.insert("flr");
    assert_eq!(picker.accept(), Some(Target::Name("flair".into())));
    picker.insert("no-match");
    assert_eq!(picker.accept(), None);
}

#[test]
fn overlay_shows_search_results_and_clear_empty_state() {
    let mut picker = picker();
    let mut screen = grid(80, 24);
    picker.draw(&mut screen, &Config::default().theme());
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Sessionizer"))
    );
    assert!(screen.to_lines().iter().any(|line| line.contains("flair")));
    assert!(screen.cursor.is_none());
    picker.insert("missing");
    picker.draw(&mut screen, &Config::default().theme());
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("No matches"))
    );
}

#[test]
fn overlay_clips_to_tiny_grids_without_changing_dimensions() {
    for (cols, rows) in [(0, 0), (1, 1), (8, 3), (20, 8)] {
        let picker = picker();
        let mut screen = grid(cols, rows);
        picker.draw(&mut screen, &Config::default().theme());
        assert_eq!(screen.cells.len(), usize::from(cols) * usize::from(rows));
        assert_eq!(screen.size, GridSize { cols, rows });
    }
}

#[test]
fn long_query_paths_and_unicode_backspace_remain_bounded() {
    let mut picker = picker();
    picker.insert("é");
    picker.backspace();
    assert!(picker.query.is_empty());
    picker.insert(&"q".repeat(1000));
    let mut screen = grid(40, 12);
    picker.draw(&mut screen, &Config::default().theme());
    assert!(
        screen
            .to_lines()
            .iter()
            .all(|line| line.chars().count() <= 40)
    );
}

#[test]
fn picker_hints_match_available_actions_and_fit_compact_width() {
    let mut picker = picker();
    let mut screen = grid(40, 12);
    picker.draw(&mut screen, &Config::default().theme());
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Enter last: previous"))
    );
    picker.insert("missing");
    let mut screen = grid(40, 12);
    picker.draw(&mut screen, &Config::default().theme());
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Edit search  Esc cancel"))
    );
    assert!(
        !screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Enter switch"))
    );
    let mut narrow = grid(20, 12);
    picker.draw(&mut narrow, &Config::default().theme());
    assert!(
        narrow
            .to_lines()
            .iter()
            .any(|line| line.contains("Edit / Esc"))
    );
}

fn sessionizer() -> Picker {
    Picker::new(
        vec![
            SessionInfo {
                name: "main".into(),
                root: "/home/u".into(),
                windows: 1,
            },
            SessionInfo {
                name: "work".into(),
                root: "/home/u/work".into(),
                windows: 2,
            },
        ],
        "main",
        None,
        vec![
            "/home/u/projects/rust-catchers".into(),
            "/home/u/projects/gitlab-registry-cleanup".into(),
        ],
    )
}

fn rendered(picker: &Picker, cols: u16, rows: u16) -> Vec<String> {
    let mut screen = grid(cols, rows);
    picker.draw(&mut screen, &Config::default().theme());
    screen.to_lines()
}

#[test]
fn sessionizer_rows_mark_open_sessions_without_dir_labels() {
    let lines = rendered(&sessionizer(), 90, 20);
    assert!(lines.iter().all(|line| !line.contains("dir ")));
    assert!(lines.iter().all(|line| !line.contains("open")));
    let work = lines.iter().find(|line| line.contains("work")).unwrap();
    assert!(work.starts_with(">* work "));
    let catchers = lines
        .iter()
        .find(|line| line.contains("rust-catchers"))
        .unwrap();
    assert!(catchers.starts_with("   rust-catchers "));
    let cleanup = lines
        .iter()
        .find(|line| line.contains("gitlab-registry-cleanup"))
        .unwrap();
    let path_column = |line: &str, path: &str| line.find(path).unwrap();
    assert_eq!(
        path_column(work, "/home/u/work"),
        path_column(catchers, "/home/u/projects/rust-catchers")
    );
    assert_eq!(
        path_column(work, "/home/u/work"),
        path_column(cleanup, "/home/u/projects/gitlab")
    );
}

#[test]
fn compact_sessionizer_rows_keep_names_and_path_tails() {
    let lines = rendered(&sessionizer(), 30, 8);
    assert!(lines.iter().all(|line| !line.contains("dir ")));
    assert!(lines.iter().all(|line| line.chars().count() <= 30));
    let work = lines.iter().find(|line| line.contains("work")).unwrap();
    assert!(work.starts_with(">* work "));
    assert!(work.ends_with("/home/u/work"));
    let catchers = lines
        .iter()
        .find(|line| line.contains("rust-catchers"))
        .unwrap();
    assert!(catchers.starts_with("   rust-catchers "));
    assert_eq!(work.find('/'), catchers.find("..."));
}
