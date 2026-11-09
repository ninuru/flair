use super::*;
use flair_core::{
    config::Config,
    geom::{CellPoint, CellRect, GridSize},
    proto::SessionInfo,
};

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

fn full(size: GridSize) -> CellRect {
    CellRect {
        origin: CellPoint::default(),
        size,
    }
}

fn draw(picker: &Picker, grid: &mut CellGrid) -> CellRect {
    let area = full(grid.size);
    picker.draw(grid, area, &Config::default().theme())
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
    draw(&picker, &mut screen);
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Sessionizer"))
    );
    assert!(screen.to_lines().iter().any(|line| line.contains("flair")));
    assert!(screen.cursor.is_none());
    picker.insert("missing");
    draw(&picker, &mut screen);
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
        draw(&picker, &mut screen);
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
    draw(&picker, &mut screen);
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
    draw(&picker, &mut screen);
    assert!(
        screen
            .to_lines()
            .iter()
            .any(|line| line.contains("Enter last: previous"))
    );
    picker.insert("missing");
    let mut screen = grid(40, 12);
    draw(&picker, &mut screen);
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
    draw(&picker, &mut narrow);
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
    draw(picker, &mut screen);
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

fn chooser() -> Picker {
    Picker::sessions(
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
            SessionInfo {
                name: "rust-catchers".into(),
                root: "/home/u/projects/rust-catchers".into(),
                windows: 1,
            },
        ],
        "work",
    )
}

#[test]
fn session_chooser_lists_every_session_and_preselects_current() {
    let mut picker = chooser();
    assert_eq!(picker.accept(), Some(Target::Name("work".into())));
    let lines = rendered(&picker, 90, 20);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("Sessions  3 matches"))
    );
    assert!(lines.iter().all(|line| !line.contains("Sessionizer")));
    let marked: Vec<_> = lines
        .iter()
        .filter(|line| line.get(1..2) == Some("*"))
        .collect();
    assert_eq!(marked.len(), 1);
    assert!(marked[0].starts_with(">* work "));
    assert!(lines.iter().any(|line| line.starts_with("   main ")));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("   rust-catchers "))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Enter switch  Up/Down move  Esc cancel"))
    );
    picker.move_selection(-1);
    assert_eq!(picker.accept(), Some(Target::Name("main".into())));
    picker.insert("rust");
    assert_eq!(picker.accept(), Some(Target::Name("rust-catchers".into())));
}

#[test]
fn compact_session_chooser_keeps_current_marker_and_hint() {
    let lines = rendered(&chooser(), 30, 8);
    assert!(lines.iter().all(|line| line.chars().count() <= 30));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("Sessions  3 matches"))
    );
    assert!(lines.iter().any(|line| line.starts_with(">* work ")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Enter switch  Esc cancel"))
    );
}

fn area() -> CellRect {
    CellRect {
        origin: CellPoint { col: 10, row: 3 },
        size: GridSize { cols: 50, rows: 12 },
    }
}

#[test]
fn draw_stays_inside_the_area_and_returns_the_lifted_rect() {
    let theme = Config::default().theme();
    let mut screen = grid(80, 24);
    let before = screen.clone();
    let lifted = chooser().draw(&mut screen, area(), &theme);
    assert_eq!(
        lifted,
        CellRect {
            origin: CellPoint { col: 10, row: 5 },
            size: GridSize { cols: 50, rows: 7 },
        }
    );
    for row in 0..24 {
        for col in 0..80 {
            let index = usize::from(row) * 80 + usize::from(col);
            if !lifted.contains(CellPoint { col, row }) {
                assert_eq!(screen.cells[index], before.cells[index]);
            }
        }
    }
}

#[test]
fn draw_centres_a_capped_width_inside_a_wide_area() {
    let mut screen = grid(120, 30);
    let lifted = draw(&chooser(), &mut screen);
    assert_eq!(lifted.size.cols, 90);
    assert_eq!(lifted.origin.col, 15);
    assert_eq!(lifted.origin.row, (30 - lifted.size.rows) / 2);
}

#[test]
fn lifted_cells_use_surface1_and_the_selected_row_surface3() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut screen = grid(80, 24);
    let lifted = chooser().draw(&mut screen, area(), &theme);
    let lines = screen.to_lines();
    let selected = lines
        .iter()
        .position(|line| line.contains(">* work"))
        .unwrap();
    for row in lifted.origin.row..lifted.origin.row + lifted.size.rows {
        for col in lifted.origin.col..lifted.origin.col + lifted.size.cols {
            let cell = &screen.cells[usize::from(row) * 80 + usize::from(col)];
            if usize::from(row) == selected {
                assert_eq!(cell.bg, surfaces.surface3);
                assert_eq!(cell.fg, theme.foreground);
            } else {
                assert_eq!(cell.bg, surfaces.surface1);
            }
        }
    }
    let hint = usize::from(lifted.origin.row + lifted.size.rows - 1);
    assert_eq!(
        screen.cells[hint * 80 + usize::from(lifted.origin.col)].fg,
        surfaces.muted
    );
}
