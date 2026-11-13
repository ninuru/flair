use std::{
    ffi::OsString,
    path::{Path, PathBuf},
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
use crate::explorer::{Entry, ExplorerOutcome, Listing, RowKind};

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
        panels.key(&char('j'), &keys, now),
        Some(ExplorerOutcome::Redraw)
    );
    assert_eq!(
        panels.key(&char('z'), &keys, now),
        Some(ExplorerOutcome::Ignored)
    );
    panels.focused = false;
    assert_eq!(panels.key(&char('j'), &keys, now), None);
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
    assert_eq!(panels.key(&ctrl('a'), &keys, now), None);
    assert_eq!(panels.key(&shift, &keys, now), None);
    assert_eq!(panels.key(&char('c'), &keys, now), None);
    assert_eq!(
        panels.key(&char('j'), &keys, now),
        Some(ExplorerOutcome::Redraw)
    );
    assert_eq!(panels.key(&ctrl('a'), &keys, now), None);
    let late = now + Duration::from_millis(keys.timeout_ms + 1);
    assert_eq!(
        panels.key(&char('k'), &keys, late),
        Some(ExplorerOutcome::Redraw)
    );
}

#[test]
fn root_bindings_still_reach_the_server() {
    let keys = KeysConfig::default();
    let mut panels = opened(false);
    assert_eq!(panels.key(&alt('1'), &keys, Instant::now()), None);
    let disabled = KeysConfig {
        enabled: false,
        ..KeysConfig::default()
    };
    assert_eq!(
        panels.key(&alt('1'), &disabled, Instant::now()),
        Some(ExplorerOutcome::Ignored)
    );
    assert_eq!(
        panels.key(&ctrl('a'), &disabled, Instant::now()),
        Some(ExplorerOutcome::Ignored)
    );
}

#[test]
fn keys_ignored_while_editing_the_filter_are_swallowed() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = opened(false);
    panels.key(&char('/'), &keys, now);
    assert!(panels.explorer.editing());
    assert_eq!(
        panels.key(
            &press(Key::Named(NamedKey::Tab), Mods::default()),
            &keys,
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
fn edit_splits_an_editor_in_the_listed_directory_and_blurs() {
    let mut panels = opened(false);
    assert_eq!(
        panels.act(ExplorerOutcome::Edit(PathBuf::from("/p/src/main.rs"))),
        ExplorerAct::Send(ClientMsg::Command(Command::SplitRun {
            argv: vec![
                "sh".into(),
                "-c".into(),
                "exec ${EDITOR:-vi} \"$1\"".into(),
                "sh".into(),
                "/p/src/main.rs".into(),
            ],
            cwd: PathBuf::from("/p/src"),
        }))
    );
    assert!(!panels.focused);
}

#[test]
fn copy_load_redraw_and_blur_map_through() {
    let mut panels = opened(false);
    assert_eq!(
        panels.act(ExplorerOutcome::Copy(PathBuf::from("/p/a"))),
        ExplorerAct::Copy(PathBuf::from("/p/a"))
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
    panels.key(&char('.'), &KeysConfig::default(), Instant::now());
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
