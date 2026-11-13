use super::*;
use flair_core::key::{Locks, Mods};

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

fn listing(dir: &str, entries: &[(&str, bool)]) -> Listing {
    Listing {
        dir: PathBuf::from(dir),
        entries: entries
            .iter()
            .map(|(name, dir)| Entry {
                name: OsString::from(name),
                dir: *dir,
            })
            .collect(),
        more: 0,
        error: None,
    }
}

fn paths(items: &[&str]) -> Vec<PathBuf> {
    items.iter().map(PathBuf::from).collect()
}

fn lines(explorer: &Explorer) -> Vec<String> {
    explorer
        .rows()
        .iter()
        .map(|row| {
            let indent = "  ".repeat(usize::from(row.depth));
            let label = match &row.kind {
                RowKind::Dir { open: true } => format!("v {}", row.name),
                RowKind::Dir { open: false } => format!("> {}", row.name),
                RowKind::File => row.name.clone(),
                RowKind::More(count) => format!("+{count} more"),
                RowKind::Loading => "...".to_string(),
                RowKind::Error(error) => format!("! {error}"),
            };
            format!("{indent}{label}")
        })
        .collect()
}

fn selected_name(explorer: &Explorer) -> String {
    let index = explorer.selected().unwrap();
    explorer.rows()[index].name.clone()
}

fn project() -> Explorer {
    let mut explorer = Explorer::new(true);
    assert_eq!(explorer.set_root(Path::new("/p")), paths(&["/p"]));
    assert!(explorer.insert(listing(
        "/p",
        &[
            (".git", true),
            ("src", true),
            ("target", true),
            (".env", false),
            ("Cargo.toml", false),
            ("README.md", false),
        ],
    )));
    explorer
}

fn expand_src(explorer: &mut Explorer) {
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    assert_eq!(selected_name(explorer), "src");
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert!(explorer.insert(listing(
        "/p/src",
        &[("bin", true), ("lib.rs", false), ("main.rs", false)],
    )));
}

#[test]
fn a_new_explorer_has_no_root_and_no_rows() {
    let explorer = Explorer::new(true);
    assert_eq!(explorer.root(), None);
    assert!(explorer.rows().is_empty());
    assert_eq!(explorer.selected(), None);
    assert_eq!(explorer.filter(), None);
    assert!(!explorer.editing());
}

#[test]
fn the_root_is_row_zero_and_loads_with_one_loading_row() {
    let mut explorer = Explorer::new(true);
    assert_eq!(explorer.set_root(Path::new("/p")), paths(&["/p"]));
    assert_eq!(explorer.root(), Some(Path::new("/p")));
    assert_eq!(lines(&explorer), ["v p", "  ..."]);
    assert_eq!(explorer.selected(), Some(0));
}

#[test]
fn the_filesystem_root_is_named_slash() {
    let mut explorer = Explorer::new(true);
    explorer.set_root(Path::new("/"));
    assert_eq!(lines(&explorer)[0], "v /");
}

#[test]
fn children_follow_the_listing_order_with_depth_and_paths() {
    let explorer = project();
    assert_eq!(
        lines(&explorer),
        [
            "v p",
            "  > .git",
            "  > src",
            "  > target",
            "  .env",
            "  Cargo.toml",
            "  README.md",
        ]
    );
    let row = &explorer.rows()[5];
    assert_eq!(row.path, PathBuf::from("/p/Cargo.toml"));
    assert_eq!(row.depth, 1);
    assert!(row.matches.is_empty());
}

#[test]
fn hidden_entries_are_skipped_when_hidden_files_are_off() {
    let mut explorer = project();
    explorer.set_show_hidden(false);
    assert_eq!(
        lines(&explorer),
        [
            "v p",
            "  > src",
            "  > target",
            "  Cargo.toml",
            "  README.md"
        ]
    );
    explorer.set_show_hidden(true);
    assert_eq!(lines(&explorer).len(), 7);
}

#[test]
fn dot_toggles_hidden_files_for_this_client() {
    let mut explorer = project();
    assert_eq!(explorer.key(&char('.'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(lines(&explorer).len(), 5);
    assert_eq!(explorer.key(&char('.'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(lines(&explorer).len(), 7);
}

#[test]
fn hiding_the_selected_dotfile_moves_the_selection_to_its_parent() {
    let mut explorer = project();
    explorer.key(&char('j'), false, 20);
    assert_eq!(selected_name(&explorer), ".git");
    explorer.key(&char('.'), false, 20);
    assert_eq!(selected_name(&explorer), "p");
}

#[test]
fn right_expands_lazily_and_shows_loading_until_the_listing_arrives() {
    let mut explorer = project();
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    assert_eq!(
        explorer.key(&named(NamedKey::Right), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(&lines(&explorer)[2..4], ["  v src", "    ..."]);
    assert!(explorer.insert(listing("/p/src", &[("lib.rs", false)])));
    assert_eq!(&lines(&explorer)[2..4], ["  v src", "    lib.rs"]);
    assert_eq!(explorer.rows()[3].depth, 2);
    assert_eq!(explorer.rows()[3].path, PathBuf::from("/p/src/lib.rs"));
}

#[test]
fn right_on_an_open_folder_moves_to_its_first_child() {
    let mut explorer = project();
    expand_src(&mut explorer);
    assert_eq!(explorer.key(&char('l'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "bin");
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src/bin"]))
    );
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Ignored
    );
}

#[test]
fn right_on_the_root_moves_to_its_first_child_and_on_a_file_is_ignored() {
    let mut explorer = project();
    assert_eq!(explorer.key(&char('l'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), ".git");
    explorer.key(&named(NamedKey::End), false, 20);
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Ignored
    );
}

#[test]
fn left_collapses_an_open_folder_and_otherwise_moves_to_the_parent() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    assert_eq!(selected_name(&explorer), "lib.rs");
    assert_eq!(explorer.key(&char('h'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "src");
    assert_eq!(
        explorer.key(&named(NamedKey::Left), false, 20),
        ExplorerOutcome::Redraw
    );
    assert_eq!(&lines(&explorer)[2..4], ["  > src", "  > target"]);
    assert_eq!(explorer.key(&char('h'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "p");
    assert_eq!(
        explorer.key(&char('h'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(lines(&explorer)[0], "v p");
}

#[test]
fn collapsing_keeps_nested_expansion_for_the_next_expand() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('j'), false, 20);
    assert_eq!(selected_name(&explorer), "bin");
    explorer.key(&char('l'), false, 20);
    assert!(explorer.insert(listing("/p/src/bin", &[("tool.rs", false)])));
    explorer.key(&char('k'), false, 20);
    assert_eq!(selected_name(&explorer), "src");
    explorer.key(&char('h'), false, 20);
    assert_eq!(&lines(&explorer)[2..4], ["  > src", "  > target"]);
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(
        &lines(&explorer)[2..8],
        [
            "  v src",
            "    v bin",
            "      tool.rs",
            "    lib.rs",
            "    main.rs",
            "  > target"
        ]
    );
}

#[test]
fn every_expand_requests_a_fresh_listing_and_keeps_the_old_one_visible() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('h'), false, 20);
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(
        &lines(&explorer)[3..6],
        ["    > bin", "    lib.rs", "    main.rs"]
    );
    assert!(explorer.insert(listing("/p/src", &[("new.rs", false)])));
    assert_eq!(&lines(&explorer)[3..4], ["    new.rs"]);
}

#[test]
fn listings_that_are_not_pending_are_ignored() {
    let mut explorer = project();
    assert!(!explorer.insert(listing("/p/src", &[("lib.rs", false)])));
    assert!(!explorer.insert(listing("/p", &[])));
    assert_eq!(lines(&explorer).len(), 7);
}

#[test]
fn a_capped_listing_ends_in_a_more_row() {
    let mut explorer = Explorer::new(true);
    explorer.set_root(Path::new("/big"));
    let mut big = listing("/big", &[("a", false), ("b", false)]);
    big.more = 48_000;
    explorer.insert(big);
    assert_eq!(lines(&explorer), ["v big", "  a", "  b", "  +48000 more"]);
    assert_eq!(explorer.rows()[3].depth, 1);
}

#[test]
fn an_unreadable_directory_shows_one_error_row() {
    let mut explorer = Explorer::new(true);
    explorer.set_root(Path::new("/root"));
    let mut denied = listing("/root", &[]);
    denied.error = Some("Permission denied (os error 13)".into());
    explorer.insert(denied);
    assert_eq!(
        lines(&explorer),
        ["v root", "  ! Permission denied (os error 13)"]
    );
}

#[test]
fn the_cursor_skips_more_loading_and_error_rows() {
    let mut explorer = Explorer::new(true);
    explorer.set_root(Path::new("/p"));
    let mut capped = listing("/p", &[("a", true), ("b", false)]);
    capped.more = 3;
    explorer.insert(capped);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('l'), false, 20);
    assert_eq!(
        lines(&explorer),
        ["v p", "  v a", "    ...", "  b", "  +3 more"]
    );
    assert_eq!(explorer.key(&char('j'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "b");
    assert_eq!(
        explorer.key(&char('j'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(explorer.selected(), Some(3));
    assert_eq!(explorer.click(4, false), ExplorerOutcome::Ignored);
    assert_eq!(explorer.click(2, false), ExplorerOutcome::Ignored);
    assert_eq!(explorer.selected(), Some(3));
}

#[test]
fn up_down_page_home_and_end_move_the_selection() {
    let mut explorer = project();
    assert_eq!(explorer.key(&char('k'), false, 3), ExplorerOutcome::Ignored);
    assert_eq!(
        explorer.key(&named(NamedKey::Down), false, 3),
        ExplorerOutcome::Redraw
    );
    assert_eq!(explorer.selected(), Some(1));
    explorer.key(&named(NamedKey::Up), false, 3);
    assert_eq!(explorer.selected(), Some(0));
    explorer.key(&named(NamedKey::PageDown), false, 3);
    assert_eq!(explorer.selected(), Some(3));
    explorer.key(&named(NamedKey::PageDown), false, 3);
    assert_eq!(explorer.selected(), Some(6));
    explorer.key(&named(NamedKey::PageUp), false, 3);
    assert_eq!(explorer.selected(), Some(3));
    explorer.key(&named(NamedKey::End), false, 3);
    assert_eq!(explorer.selected(), Some(6));
    assert_eq!(
        explorer.key(&named(NamedKey::End), false, 3),
        ExplorerOutcome::Ignored
    );
    explorer.key(&named(NamedKey::Home), false, 3);
    assert_eq!(explorer.selected(), Some(0));
}

#[test]
fn moving_keeps_the_selection_on_screen() {
    let mut explorer = project();
    for _ in 0..3 {
        explorer.key(&char('j'), false, 3);
    }
    assert_eq!(explorer.scroll(), 1);
    explorer.key(&named(NamedKey::End), false, 3);
    assert_eq!(explorer.scroll(), 4);
    explorer.key(&char('k'), false, 3);
    explorer.key(&char('k'), false, 3);
    assert_eq!(explorer.scroll(), 4);
    explorer.key(&char('k'), false, 3);
    assert_eq!(explorer.selected(), Some(3));
    assert_eq!(explorer.scroll(), 3);
    explorer.key(&named(NamedKey::Home), false, 3);
    assert_eq!(explorer.scroll(), 0);
}

#[test]
fn the_wheel_scrolls_within_bounds_without_moving_the_selection() {
    let mut explorer = project();
    explorer.wheel(2, 3);
    assert_eq!(explorer.scroll(), 2);
    explorer.wheel(10, 3);
    assert_eq!(explorer.scroll(), 4);
    explorer.wheel(-10, 3);
    assert_eq!(explorer.scroll(), 0);
    explorer.wheel(5, 20);
    assert_eq!(explorer.scroll(), 0);
    assert_eq!(explorer.selected(), Some(0));
}

#[test]
fn enter_toggles_folders_and_edits_files() {
    let mut explorer = project();
    assert_eq!(
        explorer.key(&named(NamedKey::Enter), false, 20),
        ExplorerOutcome::Ignored
    );
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    assert_eq!(
        explorer.key(&named(NamedKey::Enter), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(
        explorer.key(&named(NamedKey::Enter), false, 20),
        ExplorerOutcome::Redraw
    );
    assert_eq!(lines(&explorer)[2], "  > src");
    explorer.key(&named(NamedKey::End), false, 20);
    assert_eq!(
        explorer.key(&named(NamedKey::Enter), false, 20),
        ExplorerOutcome::Edit(PathBuf::from("/p/README.md"))
    );
}

#[test]
fn action_keys_report_the_selected_path() {
    let mut explorer = project();
    explorer.key(&named(NamedKey::End), false, 20);
    let readme = PathBuf::from("/p/README.md");
    assert_eq!(
        explorer.key(&char('o'), false, 20),
        ExplorerOutcome::Edit(readme.clone())
    );
    assert_eq!(
        explorer.key(&char('i'), false, 20),
        ExplorerOutcome::Insert(readme.clone())
    );
    assert_eq!(
        explorer.key(&char('y'), false, 20),
        ExplorerOutcome::Copy(readme)
    );
    assert_eq!(explorer.key(&char('c'), true, 20), ExplorerOutcome::Ignored);
    explorer.key(&named(NamedKey::Home), false, 20);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    let src = PathBuf::from("/p/src");
    assert_eq!(explorer.key(&char('o'), true, 20), ExplorerOutcome::Ignored);
    assert_eq!(
        explorer.key(&char('c'), true, 20),
        ExplorerOutcome::Cd(src.clone())
    );
    assert_eq!(
        explorer.key(&char('c'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(
        explorer.key(&char('i'), false, 20),
        ExplorerOutcome::Insert(src.clone())
    );
    assert_eq!(
        explorer.key(&char('y'), false, 20),
        ExplorerOutcome::Copy(src)
    );
}

#[test]
fn escape_blurs_and_other_keys_are_ignored() {
    let mut explorer = project();
    assert_eq!(
        explorer.key(&named(NamedKey::Escape), false, 20),
        ExplorerOutcome::Blur
    );
    let before = lines(&explorer);
    for key in [
        char('x'),
        char('J'),
        named(NamedKey::Tab),
        named(NamedKey::Backspace),
    ] {
        assert_eq!(
            explorer.key(&key, true, 20),
            ExplorerOutcome::Ignored,
            "{key:?}"
        );
    }
    let ctrl = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..char('j')
    };
    assert_eq!(explorer.key(&ctrl, false, 20), ExplorerOutcome::Ignored);
    let release = KeyInput {
        action: KeyAction::Release,
        ..char('j')
    };
    assert_eq!(explorer.key(&release, false, 20), ExplorerOutcome::Ignored);
    assert_eq!(explorer.text("abc"), ExplorerOutcome::Ignored);
    assert_eq!(lines(&explorer), before);
    assert_eq!(explorer.selected(), Some(0));
}

#[test]
fn keys_without_a_root_are_ignored() {
    let mut explorer = Explorer::new(true);
    assert_eq!(
        explorer.key(&char('j'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(
        explorer.key(&char('i'), false, 20),
        ExplorerOutcome::Ignored
    );
    assert_eq!(explorer.click(0, false), ExplorerOutcome::Ignored);
    assert!(explorer.reload().is_empty());
}

#[test]
fn clicks_select_toggle_folders_and_double_click_edits_files() {
    let mut explorer = project();
    assert_eq!(
        explorer.click(2, false),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(selected_name(&explorer), "src");
    assert_eq!(explorer.click(2, false), ExplorerOutcome::Redraw);
    assert_eq!(lines(&explorer)[2], "  > src");
    assert_eq!(explorer.click(6, false), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "README.md");
    assert_eq!(
        explorer.click(6, true),
        ExplorerOutcome::Edit(PathBuf::from("/p/README.md"))
    );
    assert_eq!(explorer.click(0, false), ExplorerOutcome::Redraw);
    assert_eq!(lines(&explorer)[0], "v p");
    assert_eq!(explorer.click(99, false), ExplorerOutcome::Ignored);
}

#[test]
fn slash_opens_the_filter_and_typing_narrows_the_rows() {
    let mut explorer = project();
    expand_src(&mut explorer);
    assert_eq!(explorer.key(&char('/'), false, 20), ExplorerOutcome::Redraw);
    assert!(explorer.editing());
    assert_eq!(explorer.filter(), Some(""));
    assert_eq!(lines(&explorer).len(), 10);
    assert_eq!(explorer.key(&char('m'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(explorer.text("ain"), ExplorerOutcome::Redraw);
    assert_eq!(explorer.filter(), Some("main"));
    assert_eq!(lines(&explorer), ["v p", "  v src", "    main.rs"]);
    assert_eq!(explorer.rows()[2].matches, [0, 1, 2, 3]);
    assert!(explorer.rows()[1].matches.is_empty());
    assert_eq!(selected_name(&explorer), "main.rs");
}

#[test]
fn the_filter_keeps_ancestors_and_reports_match_indices() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('h'), false, 20);
    explorer.key(&char('/'), false, 20);
    explorer.text("lib");
    assert_eq!(lines(&explorer), ["v p", "  v src", "    lib.rs"]);
    assert_eq!(explorer.rows()[2].matches, [0, 1, 2]);
    explorer.key(&named(NamedKey::Backspace), false, 20);
    explorer.key(&named(NamedKey::Backspace), false, 20);
    assert_eq!(explorer.filter(), Some("l"));
    assert!(lines(&explorer).contains(&"    lib.rs".to_string()));
}

#[test]
fn the_filter_searches_only_loaded_directories_and_respects_hidden_files() {
    let mut explorer = project();
    explorer.key(&char('/'), false, 20);
    explorer.text("git");
    assert_eq!(lines(&explorer), ["v p", "  > .git"]);
    explorer.key(&named(NamedKey::Escape), false, 20);
    explorer.key(&char('.'), false, 20);
    explorer.key(&char('/'), false, 20);
    explorer.text("git");
    assert_eq!(lines(&explorer), ["v p"]);
    assert_eq!(selected_name(&explorer), "p");
}

#[test]
fn enter_or_down_leave_the_field_keeping_the_filter() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('/'), false, 20);
    explorer.text(".rs");
    assert_eq!(
        explorer.key(&named(NamedKey::Enter), false, 20),
        ExplorerOutcome::Redraw
    );
    assert!(!explorer.editing());
    assert_eq!(explorer.filter(), Some(".rs"));
    assert_eq!(selected_name(&explorer), "lib.rs");
    assert_eq!(explorer.key(&char('j'), false, 20), ExplorerOutcome::Redraw);
    assert_eq!(selected_name(&explorer), "main.rs");
    explorer.key(&char('/'), false, 20);
    assert!(explorer.editing());
    assert_eq!(explorer.filter(), Some(".rs"));
    assert_eq!(
        explorer.key(&named(NamedKey::Down), false, 20),
        ExplorerOutcome::Redraw
    );
    assert!(!explorer.editing());
    assert_eq!(explorer.filter(), Some(".rs"));
}

#[test]
fn leaving_an_empty_field_drops_the_filter() {
    let mut explorer = project();
    explorer.key(&char('/'), false, 20);
    explorer.key(&named(NamedKey::Enter), false, 20);
    assert_eq!(explorer.filter(), None);
    assert!(!explorer.editing());
}

#[test]
fn escape_in_the_field_clears_the_filter_and_restores_the_tree() {
    let mut explorer = project();
    explorer.key(&char('/'), false, 20);
    explorer.text("readme");
    assert_eq!(lines(&explorer).len(), 2);
    assert_eq!(
        explorer.key(&named(NamedKey::Escape), false, 20),
        ExplorerOutcome::Redraw
    );
    assert_eq!(explorer.filter(), None);
    assert!(!explorer.editing());
    assert_eq!(lines(&explorer).len(), 7);
    assert_eq!(selected_name(&explorer), "README.md");
}

#[test]
fn the_field_takes_letters_that_are_otherwise_commands() {
    let mut explorer = project();
    explorer.key(&char('/'), false, 20);
    for c in ['j', 'k', 'c', 'y', 'i', '.'] {
        assert_eq!(explorer.key(&char(c), true, 20), ExplorerOutcome::Redraw);
    }
    assert_eq!(explorer.filter(), Some("jkcyi."));
    assert_eq!(
        explorer.key(&named(NamedKey::Tab), false, 20),
        ExplorerOutcome::Ignored
    );
    explorer.key(&named(NamedKey::Escape), false, 20);
    assert_eq!(
        explorer.key(&named(NamedKey::Backspace), false, 20),
        ExplorerOutcome::Ignored
    );
}

#[test]
fn backspace_on_an_empty_field_is_ignored() {
    let mut explorer = project();
    explorer.key(&char('/'), false, 20);
    assert_eq!(
        explorer.key(&named(NamedKey::Backspace), false, 20),
        ExplorerOutcome::Ignored
    );
    assert!(explorer.editing());
}

#[test]
fn set_root_keeps_expansion_and_returns_only_unloaded_directories() {
    let mut explorer = project();
    expand_src(&mut explorer);
    assert_eq!(
        explorer.set_root(Path::new("/p/src")),
        Vec::<PathBuf>::new()
    );
    assert_eq!(
        lines(&explorer),
        ["v src", "  > bin", "  lib.rs", "  main.rs"]
    );
    assert_eq!(explorer.selected(), Some(0));
    assert_eq!(explorer.scroll(), 0);
    assert_eq!(explorer.set_root(Path::new("/p")), paths(&["/p"]));
    assert_eq!(lines(&explorer), ["v p", "  ..."]);
    assert!(explorer.insert(listing("/p", &[("src", true), ("x", false)])));
    assert_eq!(
        lines(&explorer),
        [
            "v p",
            "  v src",
            "    > bin",
            "    lib.rs",
            "    main.rs",
            "  x"
        ]
    );
}

#[test]
fn set_root_requests_expanded_descendants_that_are_not_loaded() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.set_root(Path::new("/q"));
    assert_eq!(explorer.set_root(Path::new("/p")), paths(&["/p", "/p/src"]));
    assert!(explorer.insert(listing("/p/src", &[("lib.rs", false)])));
    assert_eq!(lines(&explorer), ["v p", "  ..."]);
    assert!(explorer.insert(listing("/p", &[("src", true)])));
    assert_eq!(lines(&explorer), ["v p", "  v src", "    lib.rs"]);
}

#[test]
fn set_root_skips_expanded_folders_under_a_collapsed_one_until_it_expands() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('l'), false, 20);
    explorer.insert(listing("/p/src/bin", &[]));
    explorer.key(&char('k'), false, 20);
    explorer.key(&char('h'), false, 20);
    assert_eq!(lines(&explorer)[2], "  > src");
    explorer.set_root(Path::new("/q"));
    assert_eq!(explorer.set_root(Path::new("/p")), paths(&["/p"]));
    explorer.insert(listing("/p", &[("src", true)]));
    explorer.key(&char('j'), false, 20);
    assert_eq!(
        explorer.key(&char('l'), false, 20),
        ExplorerOutcome::Load(paths(&["/p/src", "/p/src/bin"]))
    );
    explorer.insert(listing("/p/src", &[("bin", true)]));
    assert_eq!(
        lines(&explorer),
        ["v p", "  v src", "    v bin", "      ..."]
    );
}

#[test]
fn a_stale_listing_from_a_previous_root_is_ignored() {
    let mut explorer = Explorer::new(true);
    explorer.set_root(Path::new("/a"));
    explorer.set_root(Path::new("/b"));
    assert!(!explorer.insert(listing("/a", &[("x", false)])));
    assert!(explorer.insert(listing("/b", &[("y", false)])));
    assert_eq!(lines(&explorer), ["v b", "  y"]);
}

#[test]
fn reload_requests_the_root_and_every_open_folder() {
    let mut explorer = project();
    expand_src(&mut explorer);
    assert_eq!(explorer.reload(), paths(&["/p", "/p/src"]));
    assert_eq!(explorer.reload(), Vec::<PathBuf>::new());
    assert_eq!(lines(&explorer).len(), 10);
}

#[test]
fn the_selection_survives_a_new_listing_and_falls_back_to_the_parent() {
    let mut explorer = project();
    expand_src(&mut explorer);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    explorer.key(&char('j'), false, 20);
    assert_eq!(selected_name(&explorer), "main.rs");
    explorer.reload();
    explorer.insert(listing("/p/src", &[("a.rs", false), ("main.rs", false)]));
    assert_eq!(selected_name(&explorer), "main.rs");
    assert_eq!(explorer.selected(), Some(4));
    explorer.reload();
    explorer.insert(listing("/p/src", &[]));
    assert_eq!(selected_name(&explorer), "src");
}

#[test]
fn quote_uses_posix_single_quotes() {
    assert_eq!(quote(Path::new("/tmp/a")), "'/tmp/a'");
    assert_eq!(quote(Path::new("/tmp/my file")), "'/tmp/my file'");
    assert_eq!(quote(Path::new("/tmp/it's")), r"'/tmp/it'\''s'");
    assert_eq!(quote(Path::new("/tmp/a\nb")), "'/tmp/a\nb'");
    assert_eq!(quote(Path::new("/tmp/\"x\" $y")), "'/tmp/\"x\" $y'");
}
