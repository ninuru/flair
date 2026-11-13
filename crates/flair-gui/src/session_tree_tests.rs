use super::*;
use flair_core::{
    ids::WindowId,
    key::{Locks, Mods},
    proto::SessionPreview,
    status::StatusWindow,
};

fn press(key: Key) -> KeyInput {
    KeyInput {
        key,
        mods: Mods::default(),
        text: match key {
            Key::Char(c) => Some(c.to_string()),
            Key::Named(_) => None,
        },
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn char(c: char) -> KeyInput {
    press(Key::Char(c))
}

fn named(key: NamedKey) -> KeyInput {
    press(Key::Named(key))
}

fn window(id: u32, index: u16, name: &str, active: bool) -> StatusWindow {
    StatusWindow {
        id: WindowId(id),
        index,
        name: name.into(),
        program: String::new(),
        active,
        last: false,
    }
}

fn node(id: u32, name: &str, windows: Vec<StatusWindow>) -> SessionNode {
    SessionNode {
        id: SessionId(id),
        name: name.into(),
        root: format!("/srv/{name}").into(),
        windows,
    }
}

fn tree() -> SessionTree {
    SessionTree::new(
        vec![
            node(
                1,
                "main",
                vec![window(10, 1, "zsh", true), window(11, 2, "nvim", false)],
            ),
            node(2, "notes", vec![window(20, 1, "notes", true)]),
            node(
                3,
                "work",
                vec![
                    window(30, 1, "a", false),
                    window(31, 2, "b", true),
                    window(32, 3, "c", false),
                ],
            ),
        ],
        SessionId(2),
        preview(2),
    )
}

fn preview(session: u32) -> SessionPreview {
    SessionPreview {
        session: SessionId(session),
        windows: Vec::new(),
    }
}

fn watch(session: u32) -> TreeOutcome {
    TreeOutcome::Watch(SessionId(session))
}

fn strip(tree: &SessionTree) -> (Vec<WindowId>, Option<SessionId>) {
    let strip = tree.sheet().strip.expect("the tree has a strip");
    (
        strip.windows.iter().map(|window| window.id).collect(),
        strip.preview.map(|preview| preview.session),
    )
}

fn names(tree: &SessionTree) -> Vec<String> {
    tree.sheet().rows.into_iter().map(|row| row.name).collect()
}

fn selected(tree: &SessionTree) -> String {
    let sheet = tree.sheet();
    sheet.rows[sheet.selected].name.clone()
}

fn switch(name: &str) -> TreeOutcome {
    TreeOutcome::Run(Command::Switch(Target::Name(name.into())))
}

fn select(id: u32) -> TreeOutcome {
    TreeOutcome::Run(Command::SelectWindow(WindowRef::Id(WindowId(id))))
}

#[test]
fn opens_on_the_current_session_with_every_session_collapsed() {
    let tree = tree();
    let sheet = tree.sheet();
    assert_eq!(sheet.title, Some("Sessions"));
    assert_eq!(sheet.field, None);
    assert_eq!(sheet.placement, Placement::Centred);
    assert_eq!(sheet.count, (3, 3));
    assert_eq!(sheet.selected, 1);
    let rows: Vec<_> = sheet
        .rows
        .iter()
        .map(|row| {
            (
                row.depth,
                row.number,
                row.name.as_str(),
                row.detail.as_deref(),
                row.label,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (0, Some(1), "main", Some("2 windows"), None),
            (0, Some(2), "notes", Some("1 window"), Some("current")),
            (0, Some(3), "work", Some("3 windows"), None),
        ]
    );
    assert_eq!(strip(&tree), (vec![WindowId(20)], Some(SessionId(2))));
    let keys: Vec<_> = sheet
        .keys
        .iter()
        .map(|(key, verb)| (*key, verb.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            (ENTER_KEY, "switch"),
            ("j k", "move"),
            ("h l", "fold"),
            ("1-9", "jump"),
            (SEARCH_KEY, "search"),
            (ESCAPE_KEY, "close"),
        ]
    );
}

#[test]
fn j_and_down_move_down_over_visible_rows_and_stop_at_the_end() {
    let mut tree = tree();
    tree.key(&char('l'));
    assert_eq!(tree.key(&char('j')), TreeOutcome::Pending);
    assert_eq!(tree.sheet().selected, 2);
    assert_eq!(tree.key(&named(NamedKey::Down)), watch(3));
    assert_eq!(selected(&tree), "work");
    tree.key(&char('j'));
    assert_eq!(tree.sheet().selected, 3);
}

#[test]
fn k_and_up_move_up_and_stop_at_the_top() {
    let mut tree = tree();
    assert_eq!(tree.key(&char('k')), watch(1));
    assert_eq!(selected(&tree), "main");
    assert_eq!(tree.key(&named(NamedKey::Up)), TreeOutcome::Pending);
    assert_eq!(tree.sheet().selected, 0);
}

#[test]
fn l_and_right_expand_a_session_then_move_to_its_first_window() {
    let mut tree = tree();
    tree.key(&char('k'));
    assert_eq!(tree.key(&char('l')), TreeOutcome::Pending);
    assert_eq!(names(&tree), ["main", "zsh", "nvim", "notes", "work"]);
    assert_eq!(tree.sheet().selected, 0);
    assert_eq!(tree.key(&named(NamedKey::Right)), TreeOutcome::Pending);
    assert_eq!(tree.sheet().selected, 1);
    assert_eq!(tree.key(&char('l')), TreeOutcome::Ignored);
    assert_eq!(tree.sheet().selected, 1);
}

#[test]
fn h_and_left_move_a_window_to_its_parent_then_collapse_the_session() {
    let mut tree = tree();
    tree.key(&char('l'));
    tree.key(&char('l'));
    assert_eq!(tree.key(&char('h')), TreeOutcome::Pending);
    assert_eq!(tree.sheet().selected, 1);
    assert_eq!(tree.key(&named(NamedKey::Left)), TreeOutcome::Pending);
    assert_eq!(names(&tree), ["main", "notes", "work"]);
    assert_eq!(tree.sheet().selected, 1);
    assert_eq!(tree.key(&char('h')), TreeOutcome::Ignored);
}

#[test]
fn digits_switch_to_the_numbered_session_at_once() {
    let mut tree = tree();
    assert_eq!(tree.key(&char('1')), switch("main"));
    assert_eq!(tree.key(&char('3')), switch("work"));
    assert_eq!(tree.key(&char('2')), TreeOutcome::Close);
    assert_eq!(tree.key(&char('9')), TreeOutcome::Ignored);
}

#[test]
fn enter_switches_sessions_closes_on_the_current_and_selects_windows() {
    let mut tree = tree();
    assert_eq!(tree.key(&named(NamedKey::Enter)), TreeOutcome::Close);
    tree.key(&char('l'));
    tree.key(&char('j'));
    assert_eq!(tree.key(&named(NamedKey::Enter)), select(20));
    tree.key(&char('j'));
    assert_eq!(tree.key(&named(NamedKey::Enter)), switch("work"));
    tree.key(&char('l'));
    tree.key(&char('l'));
    tree.key(&char('j'));
    assert_eq!(tree.key(&named(NamedKey::Enter)), select(31));
}

#[test]
fn q_and_escape_close_without_modifiers_only() {
    let mut tree = tree();
    assert_eq!(tree.key(&char('q')), TreeOutcome::Close);
    assert_eq!(tree.key(&named(NamedKey::Escape)), TreeOutcome::Close);
    let ctrl = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..char('q')
    };
    assert_eq!(tree.key(&ctrl), TreeOutcome::Ignored);
    let alt = KeyInput {
        mods: Mods {
            alt: true,
            ..Mods::default()
        },
        ..char('j')
    };
    assert_eq!(tree.key(&alt), TreeOutcome::Ignored);
    assert_eq!(tree.sheet().selected, 1);
}

#[test]
fn every_other_key_is_ignored_and_changes_nothing() {
    let mut tree = tree();
    let before = tree.sheet();
    for key in [
        char('x'),
        char('a'),
        char('J'),
        char('0'),
        named(NamedKey::Tab),
        named(NamedKey::Backspace),
        named(NamedKey::PageDown),
    ] {
        assert_eq!(tree.key(&key), TreeOutcome::Ignored, "{key:?}");
        assert_eq!(tree.sheet(), before);
    }
    let release = KeyInput {
        action: KeyAction::Release,
        ..char('j')
    };
    assert_eq!(tree.key(&release), TreeOutcome::Ignored);
    assert_eq!(tree.insert("main"), TreeOutcome::Ignored);
    assert_eq!(tree.sheet(), before);
}

#[test]
fn slash_opens_search_on_the_current_selection() {
    let mut tree = tree();
    assert!(!tree.is_searching());
    assert_eq!(tree.key(&char('/')), TreeOutcome::Pending);
    assert!(tree.is_searching());
    let sheet = tree.sheet();
    assert_eq!(sheet.title, None);
    let field = sheet.field.as_ref().unwrap();
    assert_eq!(
        (field.text.as_str(), field.placeholder),
        ("", "Search sessions")
    );
    assert_eq!(names(&tree), ["main", "notes", "work"]);
    assert_eq!(sheet.selected, 1);
    assert_eq!(sheet.count, (3, 3));
    let keys: Vec<_> = sheet
        .keys
        .iter()
        .map(|(key, verb)| (*key, verb.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            (ENTER_KEY, "switch"),
            (MOVE_KEY, "move"),
            (ESCAPE_KEY, "back")
        ]
    );
}

#[test]
fn search_typing_filters_names_with_match_highlights() {
    let mut tree = tree();
    tree.key(&char('/'));
    assert_eq!(tree.key(&char('o')), TreeOutcome::Pending);
    assert_eq!(names(&tree), ["notes", "work"]);
    assert_eq!(tree.sheet().selected, 0);
    tree.key(&char('r'));
    let sheet = tree.sheet();
    assert_eq!(names(&tree), ["work"]);
    assert_eq!(sheet.rows[0].name_matches, [1, 2]);
    assert_eq!(sheet.count, (1, 3));
    assert_eq!(sheet.field.unwrap().text, "or");
    assert_eq!(tree.key(&char('q')), TreeOutcome::Pending);
    assert!(tree.sheet().rows.is_empty());
    assert_eq!(tree.key(&named(NamedKey::Enter)), TreeOutcome::Ignored);
    tree.key(&named(NamedKey::Backspace));
    assert_eq!(tree.key(&named(NamedKey::Enter)), switch("work"));
}

#[test]
fn search_arrows_move_and_enter_switches() {
    let mut tree = tree();
    tree.key(&char('/'));
    assert_eq!(tree.key(&named(NamedKey::Up)), watch(1));
    assert_eq!(selected(&tree), "main");
    assert_eq!(tree.key(&named(NamedKey::Enter)), switch("main"));
    tree.key(&named(NamedKey::Down));
    tree.key(&named(NamedKey::Down));
    tree.key(&named(NamedKey::Down));
    assert_eq!(selected(&tree), "work");
    assert_eq!(tree.key(&char('j')), TreeOutcome::Pending);
    assert_eq!(tree.sheet().field.unwrap().text, "j");
}

#[test]
fn search_escape_clears_the_query_and_keeps_the_selected_session() {
    let mut tree = tree();
    tree.key(&char('/'));
    tree.insert("wo");
    assert_eq!(tree.key(&named(NamedKey::Escape)), TreeOutcome::Pending);
    assert!(!tree.is_searching());
    assert_eq!(tree.sheet().title, Some("Sessions"));
    assert_eq!(selected(&tree), "work");
    assert_eq!(tree.sheet().selected, 2);
    tree.key(&char('/'));
    assert_eq!(tree.sheet().field.unwrap().text, "");
    assert_eq!(selected(&tree), "work");
    tree.insert("zz");
    tree.key(&named(NamedKey::Escape));
    assert_eq!(selected(&tree), "work");
}

#[test]
fn insert_reaches_the_query_only_in_search() {
    let mut tree = tree();
    assert_eq!(tree.insert("x"), TreeOutcome::Ignored);
    tree.key(&char('/'));
    assert_eq!(tree.insert("ma"), watch(1));
    assert_eq!(names(&tree), ["main"]);
}

#[test]
fn choose_acts_on_the_clicked_row_and_step_moves_the_cursor() {
    let mut tree = tree();
    assert_eq!(tree.choose(2), switch("work"));
    assert_eq!(tree.choose(1), TreeOutcome::Close);
    tree.key(&char('l'));
    assert_eq!(tree.choose(2), select(20));
    assert_eq!(tree.choose(0), switch("main"));
    assert_eq!(tree.choose(9), TreeOutcome::Ignored);
    let mut tree = self::tree();
    assert_eq!(tree.step(2), watch(3));
    assert_eq!(selected(&tree), "work");
    tree.step(-10);
    assert_eq!(tree.sheet().selected, 0);
    tree.key(&char('/'));
    tree.insert("o");
    assert_eq!(tree.choose(1), switch("work"));
    assert_eq!(tree.choose(5), TreeOutcome::Ignored);
}

#[test]
fn moving_to_another_session_watches_it_once() {
    let mut tree = tree();
    assert_eq!(tree.key(&char('j')), watch(3));
    assert_eq!(tree.key(&char('l')), TreeOutcome::Pending);
    assert_eq!(tree.key(&char('l')), TreeOutcome::Pending);
    assert_eq!(tree.key(&char('k')), TreeOutcome::Pending);
    assert_eq!(tree.key(&char('k')), watch(2));
    assert_eq!(tree.key(&char('k')), watch(1));
}

#[test]
fn search_watches_the_selected_match() {
    let mut tree = tree();
    tree.key(&char('/'));
    assert_eq!(tree.insert("wo"), watch(3));
    assert_eq!(tree.insert("zz"), TreeOutcome::Pending);
    assert_eq!(strip(&tree), (vec![], None));
    tree.key(&named(NamedKey::Backspace));
    assert_eq!(tree.key(&named(NamedKey::Backspace)), TreeOutcome::Pending);
}

#[test]
fn previews_for_other_sessions_are_ignored() {
    let mut tree = tree();
    assert_eq!(strip(&tree), (vec![WindowId(20)], Some(SessionId(2))));
    assert!(!tree.set_preview(preview(3)));
    assert_eq!(strip(&tree).1, Some(SessionId(2)));
    tree.key(&named(NamedKey::Down));
    tree.key(&named(NamedKey::Down));
    assert_eq!(
        strip(&tree),
        (vec![WindowId(30), WindowId(31), WindowId(32)], None)
    );
    assert!(!tree.set_preview(preview(2)));
    assert!(tree.set_preview(preview(3)));
    assert_eq!(strip(&tree).1, Some(SessionId(3)));
}

#[test]
fn a_window_row_shows_the_strip_of_its_session() {
    let mut tree = tree();
    tree.key(&char('l'));
    tree.key(&char('j'));
    assert_eq!(strip(&tree), (vec![WindowId(20)], Some(SessionId(2))));
}

#[test]
fn search_orders_matches_by_score_and_keeps_their_numbers() {
    let mut tree = SessionTree::new(
        vec![
            node(1, "main", vec![window(10, 1, "zsh", true)]),
            node(2, "a-b-c", vec![window(20, 1, "zsh", true)]),
            node(3, "x-a-b-c", vec![window(30, 1, "zsh", true)]),
            node(4, "abc", vec![window(40, 1, "zsh", true)]),
        ],
        SessionId(1),
        preview(1),
    );
    tree.key(&char('/'));
    tree.insert("abc");
    let sheet = tree.sheet();
    let rows: Vec<_> = sheet
        .rows
        .iter()
        .map(|row| (row.number, row.name.as_str()))
        .collect();
    assert_eq!(
        rows,
        [(Some(4), "abc"), (Some(2), "a-b-c"), (Some(3), "x-a-b-c")]
    );
    assert_eq!(tree.key(&named(NamedKey::Enter)), switch("abc"));
}

#[test]
fn window_rows_carry_the_process_icon_and_session_rows_none() {
    let mut tree = SessionTree::new(
        vec![node(
            1,
            "main",
            vec![
                StatusWindow {
                    program: "nvim".into(),
                    ..window(10, 1, "nvim notes.md", true)
                },
                window(11, 2, "sleep", false),
            ],
        )],
        SessionId(1),
        preview(1),
    );
    assert_eq!(tree.sheet().rows.len(), 1);
    tree.key(&char('l'));
    let icons: Vec<Option<char>> = tree.sheet().rows.iter().map(|row| row.icon).collect();
    assert_eq!(icons, [None, Some('\u{e62b}'), None]);
}
