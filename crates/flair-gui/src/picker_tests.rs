use super::*;
use crate::ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY};

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

fn verbs(sheet: &Sheet) -> Vec<(&str, &str)> {
    sheet
        .keys
        .iter()
        .map(|(key, verb)| (*key, verb.as_str()))
        .collect()
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
fn sessionizer_sheet_names_the_last_session_and_marks_open_sessions() {
    let mut picker = sessionizer();
    let sheet = picker.sheet(Some(Path::new("/home/u")));
    assert_eq!(sheet.title, None);
    assert_eq!(sheet.count, (3, 3));
    assert_eq!(sheet.placement, Placement::Centred);
    assert_eq!(
        sheet.field.as_ref().map(|field| field.placeholder),
        Some("Search projects")
    );
    let row = |name: &str| sheet.rows.iter().find(|row| row.name == name).unwrap();
    assert_eq!(row("work").label, Some("open"));
    assert_eq!(row("work").detail.as_deref(), Some("~/work"));
    assert!(
        sheet
            .rows
            .iter()
            .all(|row| row.number.is_none() && row.depth == 0)
    );
    assert_eq!(row("rust-catchers").label, None);
    assert_eq!(
        row("rust-catchers").detail.as_deref(),
        Some("~/projects/rust-catchers")
    );
    assert!(sheet.rows.iter().all(|row| row.name != "main"));
    assert_eq!(
        verbs(&sheet),
        [
            (ENTER_KEY, "switch"),
            (MOVE_KEY, "move"),
            (ESCAPE_KEY, "close")
        ]
    );
    let mut last = self::picker();
    assert_eq!(verbs(&last.sheet(None))[0], (ENTER_KEY, "last: previous"));
    last.insert("f");
    assert_eq!(verbs(&last.sheet(None))[0], (ENTER_KEY, "switch"));
    picker.insert("zzqqx");
    assert!(picker.sheet(None).rows.is_empty());
}

#[test]
fn target_picks_a_ranked_row_without_the_last_session_shortcut() {
    let picker = picker();
    assert_eq!(picker.target(0), Some(Target::Name("flair".into())));
    assert_eq!(picker.target(1), None);
    let sessionizer = sessionizer();
    let index = sessionizer
        .sheet(None)
        .rows
        .iter()
        .position(|row| row.name == "rust-catchers")
        .unwrap();
    assert_eq!(
        sessionizer.target(index),
        Some(Target::Path("/home/u/projects/rust-catchers".into()))
    );
}

#[test]
fn display_path_abbreviates_home_and_drops_control_characters() {
    let home = Some(Path::new("/home/u"));
    assert_eq!(display_path(Path::new("/home/u"), home), "~");
    assert_eq!(display_path(Path::new("/home/u/a/b"), home), "~/a/b");
    assert_eq!(display_path(Path::new("/home/user"), home), "/home/user");
    assert_eq!(display_path(Path::new("/tmp/a\nb"), None), "/tmp/ab");
}

#[test]
fn query_edits_mid_text_and_the_sheet_keeps_the_cursor() {
    let mut picker = picker();
    picker.insert("fair");
    picker.query.home();
    picker.query.right();
    picker.insert("l");
    assert_eq!(picker.query.text(), "flair");
    assert_eq!(picker.accept(), Some(Target::Name("flair".into())));
    let field = picker.sheet(None).field.unwrap();
    assert_eq!((field.text.as_str(), field.cursor), ("flair", 2));
    picker.backspace();
    assert_eq!(picker.query.text(), "fair");
    picker.query.end();
    picker.query.left();
    picker.clear();
    assert_eq!(picker.query.text(), "r");
}

#[test]
fn long_queries_and_unicode_backspace_remain_bounded() {
    let mut picker = picker();
    picker.insert("é");
    picker.backspace();
    assert!(picker.query.is_empty());
    picker.insert(&"q".repeat(1000));
    assert!(picker.sheet(None).rows.is_empty());
    assert_eq!(picker.accept(), None);
}

#[test]
fn enter_verb_opens_a_directory_and_switches_to_an_open_session() {
    let mut picker = sessionizer();
    picker.insert("rust");
    let sheet = picker.sheet(None);
    assert_eq!(sheet.rows[sheet.selected].name, "rust-catchers");
    assert_eq!(verbs(&sheet)[0], (ENTER_KEY, "open"));
    assert_eq!(sheet.count, (2, 3));
    picker.clear();
    picker.insert("work");
    assert_eq!(verbs(&picker.sheet(None))[0], (ENTER_KEY, "switch"));
    picker.insert("zzqqx");
    assert_eq!(verbs(&picker.sheet(None))[0], (ENTER_KEY, "switch"));
}

#[test]
fn sheet_rows_carry_name_matches_and_path_matches_through_the_home_shortening() {
    let mut picker = sessionizer();
    picker.insert("rc");
    let sheet = picker.sheet(Some(Path::new("/home/u")));
    let row = sheet
        .rows
        .iter()
        .find(|row| row.name == "rust-catchers")
        .unwrap();
    assert_eq!(row.name_matches, [0, 5]);
    assert!(row.detail_matches.is_empty());
    let mut picker = sessionizer();
    picker.insert("projects");
    let sheet = picker.sheet(Some(Path::new("/home/u")));
    let row = &sheet.rows[0];
    assert_eq!(row.detail.as_deref(), Some("~/projects/rust-catchers"));
    assert!(row.name_matches.is_empty());
    assert_eq!(row.detail_matches, (2..10).collect::<Vec<_>>());
}

#[test]
fn display_matches_follow_the_home_shortening_and_dropped_control_characters() {
    let home = Some(Path::new("/home/u"));
    assert_eq!(
        display_matches(Path::new("/home/u/ab"), home, &[1, 8, 9]),
        ("~/ab".to_owned(), vec![2, 3])
    );
    assert_eq!(
        display_matches(Path::new("/x\ny"), None, &[3]),
        ("/xy".to_owned(), vec![2])
    );
    assert_eq!(
        display_matches(Path::new("/home/u"), home, &[0, 1]),
        ("~".to_owned(), vec![])
    );
}
