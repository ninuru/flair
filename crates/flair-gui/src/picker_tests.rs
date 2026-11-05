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
