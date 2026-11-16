use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use flair_core::{
    agent::AgentState,
    config::Side,
    geom::Direction,
    ids::{PaneId, WindowId},
    key::{Key, KeyAction, KeyInput, Locks, Mods},
    keys::KeysConfig,
    status::ActivePane,
};

use super::super::*;
use crate::status::AgentParts;

fn press(c: char, mods: Mods) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods,
        text: Some(c.to_string()),
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
    press(c, Mods::default())
}

fn alt(c: char) -> KeyInput {
    press(
        c,
        Mods {
            alt: true,
            ..Mods::default()
        },
    )
}

fn ctrl(c: char) -> KeyInput {
    press(
        c,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    )
}

fn agents() -> Vec<AgentParts> {
    [(1, AgentState::Working), (2, AgentState::Blocked)]
        .into_iter()
        .map(|(pane, state)| AgentParts {
            pane: PaneId(pane),
            window: WindowId(pane + 10),
            session: "main".into(),
            icon: None,
            title: format!("agent {pane}"),
            label: state.label(None),
            state,
            unseen: false,
            active: false,
        })
        .collect()
}

fn files() -> Panels {
    let mut panels = Panels::default();
    panels.follow(Some(ActivePane {
        pane: PaneId(1),
        cwd: Some(PathBuf::from("/p")),
        at_prompt: true,
    }));
    panels.explorer_command();
    panels.focused = false;
    panels
}

fn agents_panel() -> Panels {
    let mut panels = files();
    panels.agents_command(&agents());
    panels.focused = false;
    panels.agent = Some(PaneId(1));
    panels
}

#[test]
fn select_pane_toward_files_at_the_edge_focuses_them() {
    let mut panels = files();
    assert!(panels.edge(Direction::Left, Side::Left, &agents()));
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);

    let mut panels = files();
    assert!(panels.edge(Direction::Right, Side::Right, &agents()));
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
}

#[test]
fn select_pane_toward_agents_at_the_edge_focuses_them_like_the_agents_command() {
    let mut panels = agents_panel();
    assert!(panels.edge(Direction::Left, Side::Left, &agents()));
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
    assert_eq!(panels.agent, Some(PaneId(2)));

    let mut panels = agents_panel();
    assert!(panels.edge(Direction::Right, Side::Right, &agents()));
    assert!(panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
}

#[test]
fn select_pane_away_from_or_across_the_panel_does_not_focus_it() {
    for direction in [Direction::Right, Direction::Up, Direction::Down] {
        let mut panels = files();
        let before = panels.clone();
        assert!(!panels.edge(direction, Side::Left, &agents()));
        assert_eq!(panels, before);
    }
    let mut panels = agents_panel();
    let before = panels.clone();
    assert!(!panels.edge(Direction::Left, Side::Right, &agents()));
    assert_eq!(panels, before);
}

#[test]
fn a_closed_panel_or_the_windows_list_never_takes_focus() {
    let mut panels = Panels::default();
    assert!(!panels.edge(Direction::Left, Side::Left, &agents()));
    assert!(!panels.open && !panels.focused);

    let mut panels = files();
    panels.explorer_command();
    panels.explorer_command();
    assert!(!panels.open);
    assert!(!panels.edge(Direction::Left, Side::Left, &agents()));
    assert!(!panels.open && !panels.focused);

    let mut panels = files();
    panels.switch(PanelView::Tabs, &agents());
    assert!(!panels.edge(Direction::Left, Side::Left, &agents()));
    assert!(!panels.focused);
    assert_eq!(panels.shown, PanelView::Tabs);
}

#[test]
fn an_already_focused_panel_keeps_its_state() {
    let mut panels = agents_panel();
    panels.focused = true;
    let before = panels.clone();
    assert!(!panels.edge(Direction::Left, Side::Left, &agents()));
    assert_eq!(panels, before);
}

#[test]
fn select_pane_toward_the_terminal_blurs_the_focused_panel() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = files();
    panels.focused = true;
    assert!(panels.leave(&alt('l'), &keys, false, now, Side::Left));
    assert!(panels.open && !panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);

    let mut panels = agents_panel();
    panels.focused = true;
    assert!(panels.leave(&alt('h'), &keys, false, now, Side::Right));
    assert!(panels.open && !panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
}

#[test]
fn other_directions_and_plain_keys_keep_the_panel_focused() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    for key in [alt('h'), alt('j'), alt('k'), char('l'), char('h')] {
        let mut panels = files();
        panels.focused = true;
        assert!(!panels.leave(&key, &keys, false, now, Side::Left));
        assert!(panels.focused);
    }
    let mut panels = files();
    panels.focused = true;
    let mut release = alt('l');
    release.action = KeyAction::Release;
    assert!(!panels.leave(&release, &keys, false, now, Side::Left));
    let mut repeat = alt('l');
    repeat.repeat = true;
    assert!(!panels.leave(&repeat, &keys, false, now, Side::Left));
    assert!(panels.focused);
}

#[test]
fn an_unfocused_or_hidden_panel_ignores_select_pane() {
    let keys = KeysConfig::default();
    let mut panels = files();
    assert!(!panels.leave(&alt('l'), &keys, false, Instant::now(), Side::Left));
    assert!(!panels.focused);

    let mut panels = files();
    panels.switch(PanelView::Tabs, &agents());
    panels.focused = true;
    assert!(!panels.leave(&alt('l'), &keys, false, Instant::now(), Side::Left));
}

#[test]
fn the_prefix_binding_toward_the_terminal_blurs_the_panel() {
    let keys = KeysConfig::default();
    let now = Instant::now();
    let mut panels = files();
    panels.focused = true;
    assert!(!panels.leave(&ctrl('a'), &keys, false, now, Side::Right));
    assert_eq!(panels.key(&ctrl('a'), &keys, false, now), None);
    assert!(panels.leave(&char('h'), &keys, false, now, Side::Right));
    assert!(!panels.focused);

    let mut panels = agents_panel();
    panels.focused = true;
    assert!(panels.leave(&char('h'), &keys, true, now, Side::Right));
    assert!(!panels.focused);

    let mut panels = files();
    panels.focused = true;
    assert!(!panels.leave(&char('l'), &keys, true, now, Side::Left));
    assert!(panels.focused);
}

#[test]
fn custom_bindings_decide_by_command_not_by_key() {
    let keys = KeysConfig {
        root: BTreeMap::from([("A-x".to_owned(), "select-pane-right".to_owned())]),
        ..KeysConfig::default()
    };
    let now = Instant::now();
    let mut panels = files();
    panels.focused = true;
    assert!(panels.leave(&alt('x'), &keys, false, now, Side::Left));
    assert!(!panels.focused);

    let disabled = KeysConfig {
        enabled: false,
        ..KeysConfig::default()
    };
    let mut panels = files();
    panels.focused = true;
    assert!(!panels.leave(&alt('l'), &disabled, false, now, Side::Left));
    assert!(panels.focused);
}
