use std::{collections::BTreeMap, time::Instant};

use flair_core::{
    config::{Config, Side, TabsPosition},
    geom::{CellPoint, CellRect, GridSize},
    ids::{PaneId, WindowId},
    key::{Key, KeyAction, KeyInput, Locks, Mods, NamedKey},
    keys::KeysConfig,
    proto::{PanePlacement, WindowLayout},
};

use super::super::*;
use crate::{
    CellMetrics,
    chrome_layout::{Frame, frame},
};

fn input(key: Key, alt: bool) -> KeyInput {
    KeyInput {
        key,
        mods: Mods {
            alt,
            ..Mods::default()
        },
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

fn alt(c: char) -> KeyInput {
    input(Key::Char(c), true)
}

fn config(side: Side, position: TabsPosition) -> Config {
    let mut config = Config::default();
    config.explorer.side = side;
    config.tabs.position = position;
    config
}

fn config_left() -> Config {
    config(Side::Left, TabsPosition::Top)
}

fn dock_frame(panels: &Panels, config: &Config) -> Frame {
    frame(
        (1000, 600),
        1.0,
        false,
        (8.0, 4.0),
        CellMetrics {
            width: 8.0,
            height: 16.0,
            baseline: 12.0,
        },
        panels.dock(config, false),
    )
}

fn layout(active: u32) -> WindowLayout {
    let rect = |col, cols| CellRect {
        origin: CellPoint { col, row: 0 },
        size: GridSize { cols, rows: 10 },
    };
    WindowLayout {
        window: WindowId(1),
        area: rect(0, 21),
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(0, 10),
                band: 0,
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(11, 10),
                band: 0,
            },
        ],
        separators: vec![],
        active: PaneId(active),
        zoomed: false,
    }
}

fn agents_panel() -> Panels {
    let mut panels = Panels::default();
    panels.agents_command(&[]);
    panels.focused = false;
    panels
}

fn nav(panels: &mut Panels, key: &KeyInput, config: &Config, layout: &WindowLayout) -> bool {
    let frame = dock_frame(panels, config);
    panels.nav(key, &config.keys, false, Instant::now(), &frame, layout)
}

#[test]
fn select_pane_left_from_the_leftmost_pane_focuses_a_left_panel() {
    let config = config_left();
    let mut panels = agents_panel();
    assert!(nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(panels.focused);
    assert!(panels.has_focus());
}

#[test]
fn select_pane_left_from_an_inner_pane_is_left_to_the_server() {
    let config = config_left();
    let mut panels = agents_panel();
    assert!(!nav(&mut panels, &alt('h'), &config, &layout(2)));
    assert!(!panels.focused);
}

#[test]
fn select_pane_right_and_escape_return_from_a_left_panel() {
    let config = config_left();
    let mut panels = agents_panel();
    assert!(nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(!nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(!nav(&mut panels, &alt('j'), &config, &layout(1)));
    assert!(panels.focused);
    assert!(nav(&mut panels, &alt('l'), &config, &layout(1)));
    assert!(!panels.focused);
    assert!(nav(&mut panels, &alt('h'), &config, &layout(1)));
    let outcome = panels.agents_key(
        &input(Key::Named(NamedKey::Escape), false),
        &config.keys,
        false,
        Instant::now(),
        &[],
    );
    assert_eq!(outcome, Some(AgentsOutcome::Blur));
    assert!(!panels.focused);
}

#[test]
fn a_right_panel_mirrors_the_edge_navigation() {
    let config = config(Side::Right, TabsPosition::Top);
    let mut panels = agents_panel();
    assert!(!nav(&mut panels, &alt('l'), &config, &layout(1)));
    assert!(!nav(&mut panels, &alt('h'), &config, &layout(2)));
    assert!(nav(&mut panels, &alt('l'), &config, &layout(2)));
    assert!(panels.focused);
    assert!(nav(&mut panels, &alt('h'), &config, &layout(2)));
    assert!(!panels.focused);
}

#[test]
fn the_files_view_is_entered_like_the_agents_view() {
    let config = config_left();
    let mut panels = Panels::default();
    panels.explorer_command();
    panels.focused = false;
    assert!(nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(panels.focused && panels.has_focus());
}

#[test]
fn the_tabs_view_and_a_closed_panel_are_never_entered() {
    let config = config(Side::Left, TabsPosition::Left);
    let mut panels = agents_panel();
    panels.switch(PanelView::Tabs, &[]);
    assert!(!nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(!panels.focused && !panels.has_focus());
    let config = config_left();
    let mut closed = Panels::default();
    assert!(!nav(&mut closed, &alt('h'), &config, &layout(1)));
    assert!(!closed.focused);
}

#[test]
fn the_binding_comes_from_the_keys_config() {
    let mut config = config_left();
    config.keys.root = BTreeMap::from([
        ("A-h".to_owned(), "none".to_owned()),
        ("A-y".to_owned(), "select-pane-left".to_owned()),
    ]);
    let mut panels = agents_panel();
    assert!(!nav(&mut panels, &alt('h'), &config, &layout(1)));
    assert!(nav(&mut panels, &alt('y'), &config, &layout(1)));
    config.keys.enabled = false;
    panels.focused = false;
    assert!(!nav(&mut panels, &alt('y'), &config, &layout(1)));
}

#[test]
fn a_pending_prefix_or_a_release_is_left_alone() {
    let config = config_left();
    let mut panels = agents_panel();
    let frame = dock_frame(&panels, &config);
    let now = Instant::now();
    assert!(!panels.nav(&alt('h'), &config.keys, true, now, &frame, &layout(1)));
    let release = KeyInput {
        action: KeyAction::Release,
        ..alt('h')
    };
    assert!(!panels.nav(&release, &config.keys, false, now, &frame, &layout(1)));
    assert!(!panels.focused);
    let defaults = KeysConfig::default();
    assert!(panels.nav(&alt('h'), &defaults, false, now, &frame, &layout(1)));
}
