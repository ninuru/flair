use std::{
    ffi::{OsStr, OsString},
    fs::{self, DirBuilder, File},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use flair_core::{
    command::Command,
    config::{Config, Side},
    ids::PaneId,
    key::{InputEvent, Key, KeyAction, KeyInput, Locks, Mods, NamedKey},
    keys::KeysConfig,
    proto::ClientMsg,
    status::ActivePane,
};

use super::super::*;
use crate::{
    explorer::{Entry, ExplorerOutcome, Listing, RowKind},
    notice::Notice,
};

fn press(key: Key, mods: Mods) -> KeyInput {
    KeyInput {
        key,
        mods,
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
    press(Key::Char(c), Mods::default())
}

fn ctrl(c: char) -> KeyInput {
    press(
        Key::Char(c),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    )
}

fn alt(c: char) -> KeyInput {
    press(
        Key::Char(c),
        Mods {
            alt: true,
            ..Mods::default()
        },
    )
}

fn pane(cwd: Option<&str>, at_prompt: bool) -> Option<ActivePane> {
    Some(ActivePane {
        pane: PaneId(1),
        cwd: cwd.map(PathBuf::from),
        at_prompt,
    })
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

fn opened(at_prompt: bool) -> Panels {
    let mut panels = Panels::default();
    assert!(panels.follow(pane(Some("/p"), at_prompt)).is_empty());
    assert_eq!(panels.explorer_command(), paths(&["/p"]));
    assert!(panels.explorer.insert(listing(
        "/p",
        &[("src", true), (".env", false), ("main.rs", false)],
    )));
    panels.place(Pages {
        tabs: 0,
        explorer: 10,
    });
    panels
}

fn rooted(root: &Path) -> Panels {
    let mut panels = Panels::default();
    let cwd = Some(ActivePane {
        pane: PaneId(1),
        cwd: Some(root.to_path_buf()),
        at_prompt: false,
    });
    panels.follow(cwd);
    panels.explorer_command();
    panels
}

fn refused(act: ExplorerAct) -> bool {
    matches!(act, ExplorerAct::Notice(Notice { error: true, .. }))
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("flair-panels-{}-{index}", process::id()));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        TempDir(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }

    fn file(&self, dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        File::create(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn names(panels: &Panels) -> Vec<String> {
    panels
        .explorer
        .rows()
        .iter()
        .map(|row| row.name.clone())
        .collect()
}

#[test]
fn the_explorer_command_opens_focuses_and_closes() {
    let mut panels = Panels::default();
    panels.follow(pane(Some("/p"), false));
    assert_eq!(panels.explorer_command(), paths(&["/p"]));
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
    assert!(panels.explorer_command().is_empty());
    assert!(!panels.open && !panels.focused);
    panels.explorer_command();
    panels.focused = false;
    panels.shown = PanelView::Tabs;
    assert!(panels.explorer_command().is_empty());
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
}

#[test]
fn the_bar_button_toggles_open_and_closed() {
    let mut panels = Panels::default();
    panels.follow(pane(Some("/p"), false));
    assert_eq!(panels.toggle_explorer(), paths(&["/p"]));
    assert!(panels.open && panels.focused);
    panels.focused = false;
    assert!(panels.toggle_explorer().is_empty());
    assert!(!panels.open && !panels.focused);
}

#[test]
fn opening_without_a_cwd_shows_no_root() {
    let mut panels = Panels::default();
    assert!(panels.explorer_command().is_empty());
    assert!(panels.open);
    assert_eq!(panels.explorer.root(), None);
}

#[test]
fn reopening_at_the_same_cwd_reloads_the_tree() {
    let mut panels = opened(false);
    panels.explorer_command();
    assert_eq!(panels.explorer_command(), paths(&["/p"]));
}

#[test]
fn the_dock_holds_the_explorer_only_while_open_and_not_in_zen() {
    let mut config = Config::default();
    config.explorer.side = Side::Right;
    config.explorer.width = 300;
    let mut panels = Panels::default();
    assert_eq!(panels.dock(&config, false).explorer, None);
    panels.explorer_command();
    assert_eq!(
        panels.dock(&config, false).explorer,
        Some((Side::Right, 300.0))
    );
    assert_eq!(panels.dock(&config, true).explorer, None);
    panels.live_width = Some((PanelView::Explorer, 420.0));
    assert_eq!(
        panels.dock(&config, false).explorer,
        Some((Side::Right, 420.0))
    );
}

#[test]
fn a_status_with_a_new_cwd_re_roots_an_open_explorer() {
    let mut panels = Panels::default();
    assert!(panels.follow(pane(Some("/a"), false)).is_empty());
    assert_eq!(panels.explorer.root(), None);
    panels.explorer_command();
    assert_eq!(panels.explorer.root(), Some(Path::new("/a")));
    assert_eq!(panels.follow(pane(Some("/b"), true)), paths(&["/b"]));
    assert_eq!(panels.explorer.root(), Some(Path::new("/b")));
    assert!(panels.follow(pane(Some("/b"), false)).is_empty());
    assert!(panels.follow(pane(None, false)).is_empty());
    assert!(panels.follow(None).is_empty());
    assert_eq!(panels.explorer.root(), Some(Path::new("/b")));
}

#[test]
fn keys_go_to_the_explorer_only_while_it_is_focused() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = opened(false);
    assert_eq!(
        panels.key(&char('j'), &keys, false, now),
        Some(ExplorerOutcome::Redraw)
    );
    assert_eq!(
        panels.key(&char('z'), &keys, false, now),
        Some(ExplorerOutcome::Ignored)
    );
    panels.focused = false;
    assert_eq!(panels.key(&char('j'), &keys, false, now), None);
    assert_eq!(panels.text("abc"), None);
}

#[test]
fn the_prefix_and_the_key_after_it_go_to_the_server() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = opened(false);
    let shift = press(
        Key::Named(NamedKey::ShiftLeft),
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(panels.key(&ctrl('a'), &keys, false, now), None);
    assert_eq!(panels.key(&shift, &keys, false, now), None);
    assert_eq!(panels.key(&char('c'), &keys, false, now), None);
    assert_eq!(
        panels.key(&char('j'), &keys, false, now),
        Some(ExplorerOutcome::Redraw)
    );
    assert_eq!(panels.key(&ctrl('a'), &keys, false, now), None);
    let late = now + Duration::from_millis(keys.timeout_ms + 1);
    assert_eq!(
        panels.key(&char('k'), &keys, false, late),
        Some(ExplorerOutcome::Redraw)
    );
}

#[test]
fn root_bindings_still_reach_the_server() {
    let keys = KeysConfig::default();
    let mut panels = opened(false);
    assert_eq!(panels.key(&alt('1'), &keys, false, Instant::now()), None);
    let disabled = KeysConfig {
        enabled: false,
        ..KeysConfig::default()
    };
    assert_eq!(
        panels.key(&alt('1'), &disabled, false, Instant::now()),
        Some(ExplorerOutcome::Ignored)
    );
    assert_eq!(
        panels.key(&ctrl('a'), &disabled, false, Instant::now()),
        Some(ExplorerOutcome::Ignored)
    );
}

#[test]
fn keys_ignored_while_editing_the_filter_are_swallowed() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = opened(false);
    panels.key(&char('/'), &keys, false, now);
    assert!(panels.explorer.editing());
    assert_eq!(
        panels.key(
            &press(Key::Named(NamedKey::Tab), Mods::default()),
            &keys,
            false,
            now
        ),
        Some(ExplorerOutcome::Ignored)
    );
    assert_eq!(panels.text("ma"), Some(ExplorerOutcome::Redraw));
    assert_eq!(panels.explorer.filter(), Some("ma"));
}

#[test]
fn insert_pastes_the_quoted_path_with_a_space() {
    let mut panels = opened(false);
    assert_eq!(
        panels.act(ExplorerOutcome::Insert(PathBuf::from("/p/it's here"))),
        ExplorerAct::Send(ClientMsg::Input(InputEvent::Paste(
            r"'/p/it'\''s here' ".into()
        )))
    );
    assert!(panels.focused);
}

#[test]
fn cd_is_sent_only_at_a_prompt() {
    let mut busy = opened(false);
    assert_eq!(
        busy.act(ExplorerOutcome::Cd(PathBuf::from("/p/src"))),
        ExplorerAct::Ignored
    );
    let mut ready = opened(true);
    assert_eq!(
        ready.act(ExplorerOutcome::Cd(PathBuf::from("/p/src"))),
        ExplorerAct::Send(ClientMsg::Input(InputEvent::Text(
            "\u{15}cd -- '/p/src'\r".into()
        )))
    );
}

#[test]
fn edit_splits_an_editor_at_the_explorer_root_and_blurs() {
    let temp = TempDir::new();
    let file = temp.file(&temp.dir("src"), "main.rs");
    let mut panels = rooted(&temp.0);
    assert_eq!(
        panels.act(ExplorerOutcome::Edit(file.clone())),
        ExplorerAct::Send(ClientMsg::Command(Command::SplitRun {
            argv: vec![
                "/bin/sh".into(),
                "-c".into(),
                "exec ${EDITOR:-vi} \"$1\"".into(),
                "flair-edit".into(),
                file.to_str().unwrap().into(),
            ],
            cwd: temp.0.clone(),
        }))
    );
    assert!(!panels.focused);
}

#[test]
fn edit_of_a_vanished_file_or_folder_shows_a_notice() {
    let temp = TempDir::new();
    let src = temp.dir("src");
    let mut panels = rooted(&temp.0);
    assert!(refused(
        panels.act(ExplorerOutcome::Edit(src.join("gone.rs")))
    ));
    let file = temp.file(&src, "main.rs");
    fs::remove_dir_all(&temp.0).unwrap();
    assert!(refused(panels.act(ExplorerOutcome::Edit(file))));
    assert!(panels.focused);
}

#[test]
fn paths_with_control_characters_never_reach_the_shell() {
    let temp = TempDir::new();
    for (index, control) in ["\u{1}", "\r", "\n", "\u{1b}", "\u{15}", "\u{3}"]
        .into_iter()
        .enumerate()
    {
        let name = format!("x{control}touch pwned{index};");
        let dir = temp.dir(&name);
        let file = temp.file(&temp.0, &format!("{name}.rs"));
        let mut panels = rooted(&temp.0);
        panels.follow(pane(temp.0.to_str(), true));
        for outcome in [
            ExplorerOutcome::Insert(dir.clone()),
            ExplorerOutcome::Cd(dir),
            ExplorerOutcome::Edit(file.clone()),
            ExplorerOutcome::Insert(file.clone()),
            ExplorerOutcome::Copy(file),
        ] {
            assert!(refused(panels.act(outcome)));
        }
        assert!(panels.focused);
    }
}

#[test]
fn paths_that_are_not_utf8_never_reach_the_server() {
    let temp = TempDir::new();
    let odd = OsStr::from_bytes(b"\xffodd");
    let dir = temp.0.join(odd);
    fs::create_dir(&dir).unwrap();
    let file = temp.file(&temp.0, "main.rs");
    let inner = temp.file(&dir, "inner.rs");
    let mut panels = rooted(&temp.0);
    panels.follow(pane(temp.0.to_str(), true));
    for outcome in [
        ExplorerOutcome::Insert(dir.clone()),
        ExplorerOutcome::Cd(dir.clone()),
        ExplorerOutcome::Copy(dir.clone()),
        ExplorerOutcome::Edit(inner.clone()),
    ] {
        assert!(refused(panels.act(outcome)));
    }
    let mut odd_root = rooted(&dir);
    assert!(refused(odd_root.act(ExplorerOutcome::Edit(inner))));
    assert!(matches!(
        panels.act(ExplorerOutcome::Edit(file)),
        ExplorerAct::Send(_)
    ));
}

#[test]
fn copy_load_redraw_and_blur_map_through() {
    let mut panels = opened(false);
    assert_eq!(
        panels.act(ExplorerOutcome::Copy(PathBuf::from("/p/a"))),
        ExplorerAct::Copy("/p/a".into())
    );
    assert_eq!(
        panels.act(ExplorerOutcome::Load(paths(&["/p/src"]))),
        ExplorerAct::Load(paths(&["/p/src"]))
    );
    assert_eq!(panels.act(ExplorerOutcome::Redraw), ExplorerAct::Redraw);
    assert_eq!(panels.act(ExplorerOutcome::Ignored), ExplorerAct::Ignored);
    assert_eq!(panels.act(ExplorerOutcome::Blur), ExplorerAct::Redraw);
    assert!(panels.open && !panels.focused);
}

#[test]
fn a_quick_second_click_on_the_same_row_is_a_double_click() {
    let mut panels = opened(false);
    panels.focused = false;
    let now = Instant::now();
    assert_eq!(
        panels.click(1, now),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert!(panels.focused);
    assert_eq!(
        panels.click(1, now + Duration::from_millis(100)),
        ExplorerOutcome::Redraw
    );
    assert_eq!(panels.explorer.rows()[1].kind, RowKind::Dir { open: true });
    let file = names(&panels)
        .iter()
        .position(|name| name == "main.rs")
        .unwrap();
    let later = now + Duration::from_secs(5);
    assert_eq!(panels.click(file, later), ExplorerOutcome::Redraw);
    assert_eq!(
        panels.click(file, later + Duration::from_millis(100)),
        ExplorerOutcome::Edit(PathBuf::from("/p/main.rs"))
    );
    let slow = later + Duration::from_secs(5);
    assert_eq!(panels.click(file, slow), ExplorerOutcome::Redraw);
    assert_eq!(
        panels.click(file, slow + Duration::from_secs(1)),
        ExplorerOutcome::Redraw
    );
}

#[test]
fn the_configured_hidden_setting_applies_when_it_changes() {
    let mut panels = opened(false);
    panels.show_hidden(true);
    assert!(names(&panels).contains(&".env".to_owned()));
    panels.show_hidden(false);
    assert!(!names(&panels).contains(&".env".to_owned()));
    panels.key(&char('.'), &KeysConfig::default(), false, Instant::now());
    assert!(names(&panels).contains(&".env".to_owned()));
    panels.show_hidden(false);
    assert!(names(&panels).contains(&".env".to_owned()));
}

#[test]
fn the_wheel_scrolls_the_tree_by_the_placed_page() {
    let mut panels = opened(false);
    panels.show_hidden(true);
    panels.place(Pages {
        tabs: 0,
        explorer: 2,
    });
    panels.scroll_explorer(5);
    assert_eq!(panels.explorer.scroll(), 2);
    panels.scroll_explorer(-1);
    assert_eq!(panels.explorer.scroll(), 1);
}

#[test]
fn keys_after_a_prefix_the_server_reports_go_to_the_server() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = opened(false);
    assert_eq!(panels.key(&char('c'), &keys, true, now), None);
    assert_eq!(
        panels.key(&char('j'), &keys, false, now),
        Some(ExplorerOutcome::Redraw)
    );
}

#[test]
fn the_explorer_loses_focus_when_its_slot_is_gone() {
    let mut panels = opened(false);
    assert!(panels.focused);
    panels.place(Pages {
        tabs: 4,
        explorer: 0,
    });
    assert!(panels.open && !panels.focused);
}

#[test]
fn a_listing_between_two_clicks_never_turns_them_into_a_double_click() {
    let mut panels = opened(false);
    let now = Instant::now();
    assert_eq!(
        panels.click(1, now),
        ExplorerOutcome::Load(paths(&["/p/src"]))
    );
    assert_eq!(panels.click(3, now), ExplorerOutcome::Redraw);
    assert!(
        panels
            .explorer
            .insert(listing("/p/src", &[("a.rs", false), ("b.rs", false)]))
    );
    assert_eq!(
        panels.click(3, now + Duration::from_millis(100)),
        ExplorerOutcome::Redraw
    );
}
