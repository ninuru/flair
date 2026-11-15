use std::{path::PathBuf, time::Instant};

use flair_core::{
    agent::AgentState,
    config::{Config, Side},
    ids::{PaneId, WindowId},
    key::{Key, KeyAction, KeyInput, Locks, Mods, NamedKey},
    keys::KeysConfig,
    status::ActivePane,
};

use super::super::*;
use crate::status::AgentParts;

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

fn agent(pane: u32, session: &str, state: AgentState, unseen: bool) -> AgentParts {
    AgentParts {
        pane: PaneId(pane),
        window: WindowId(pane + 10),
        session: session.into(),
        icon: None,
        title: format!("agent {pane}"),
        label: state.label(None),
        state,
        unseen,
        active: false,
    }
}

fn agents() -> Vec<AgentParts> {
    vec![
        agent(1, "main", AgentState::Working, false),
        agent(2, "main", AgentState::Done, true),
        agent(3, "work", AgentState::Blocked, true),
        agent(4, "work", AgentState::Idle, false),
    ]
}

fn opened() -> Panels {
    let mut panels = Panels::default();
    panels.follow(Some(ActivePane {
        pane: PaneId(1),
        cwd: Some(PathBuf::from("/p")),
        at_prompt: true,
    }));
    assert_eq!(panels.agents_command(&agents()), vec![PathBuf::from("/p")]);
    panels
}

fn key(panels: &mut Panels, key: Key, agents: &[AgentParts]) -> Option<AgentsOutcome> {
    panels.agents_key(
        &press(key),
        &KeysConfig::default(),
        false,
        Instant::now(),
        agents,
    )
}

#[test]
fn the_agents_command_opens_focuses_and_closes_the_side_panel() {
    let mut panels = opened();
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
    panels.focused = false;
    assert!(panels.agents_command(&agents()).is_empty());
    assert!(panels.open && panels.focused);
    assert!(panels.agents_command(&agents()).is_empty());
    assert!(!panels.open && !panels.focused);
}

#[test]
fn files_and_agents_switch_the_slot_without_closing_it() {
    let mut panels = opened();
    assert!(panels.explorer_command().is_empty());
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
    assert!(panels.agents_command(&agents()).is_empty());
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
    panels.explorer_command();
    panels.explorer_command();
    assert!(!panels.open);
}

#[test]
fn the_agents_view_uses_the_explorer_side_and_width() {
    let mut config = Config::default();
    config.explorer.side = Side::Right;
    config.explorer.width = 300;
    let panels = opened();
    let dock = panels.dock(&config, false);
    assert_eq!(dock.explorer, Some((Side::Right, 300.0)));
    assert_eq!(dock.shown, PanelView::Agents);
    assert_eq!(bounds(PanelView::Agents), bounds(PanelView::Explorer));
}

#[test]
fn the_cursor_starts_on_a_blocked_row_then_an_unseen_row_then_the_first() {
    let mut panels = opened();
    assert_eq!(panels.agent, Some(PaneId(3)));
    let mut rows = agents();
    rows[2].state = AgentState::Working;
    panels.agents_command(&[]);
    panels.agents_command(&rows);
    assert_eq!(panels.agent, Some(PaneId(2)));
    for row in &mut rows {
        row.unseen = false;
    }
    panels.agents_command(&[]);
    panels.agents_command(&rows);
    assert_eq!(panels.agent, Some(PaneId(1)));
    panels.agents_command(&[]);
    panels.agents_command(&[]);
    assert_eq!(panels.agent, None);
}

#[test]
fn arrows_and_vi_keys_move_the_cursor_within_the_rows() {
    let rows = agents();
    let mut panels = opened();
    assert_eq!(
        key(&mut panels, Key::Named(NamedKey::Down), &rows),
        Some(AgentsOutcome::Redraw)
    );
    assert_eq!(panels.agent, Some(PaneId(4)));
    key(&mut panels, Key::Char('j'), &rows);
    assert_eq!(panels.agent, Some(PaneId(4)));
    key(&mut panels, Key::Named(NamedKey::Up), &rows);
    key(&mut panels, Key::Char('k'), &rows);
    key(&mut panels, Key::Char('k'), &rows);
    key(&mut panels, Key::Char('k'), &rows);
    assert_eq!(panels.agent, Some(PaneId(1)));
}

#[test]
fn moving_the_cursor_scrolls_it_into_view() {
    let rows = agents();
    let mut panels = opened();
    panels.place(Pages {
        tabs: 0,
        explorer: 0,
        agents: 2,
    });
    assert_eq!(panels.agents_scroll, 3);
    key(&mut panels, Key::Char('j'), &rows);
    assert_eq!(panels.agents_scroll, 4);
    key(&mut panels, Key::Char('k'), &rows);
    key(&mut panels, Key::Char('k'), &rows);
    key(&mut panels, Key::Char('k'), &rows);
    assert_eq!(panels.agents_scroll, 1);
}

#[test]
fn the_wheel_scrolls_the_lines_and_clamps() {
    let mut panels = opened();
    panels.place(Pages {
        tabs: 0,
        explorer: 0,
        agents: 2,
    });
    panels.scroll_agents(-10, 6);
    assert_eq!(panels.agents_scroll, 0);
    panels.scroll_agents(10, 6);
    assert_eq!(panels.agents_scroll, 4);
}

#[test]
fn enter_jumps_to_the_cursor_and_returns_focus_to_the_terminal() {
    let rows = agents();
    let mut panels = opened();
    assert_eq!(
        key(&mut panels, Key::Named(NamedKey::Enter), &rows),
        Some(AgentsOutcome::Jump(WindowId(13), PaneId(3)))
    );
    assert!(panels.open && !panels.focused);
    assert_eq!(key(&mut panels, Key::Named(NamedKey::Enter), &rows), None);
}

#[test]
fn escape_blurs_and_other_keys_are_swallowed() {
    let rows = agents();
    let mut panels = opened();
    assert_eq!(
        key(&mut panels, Key::Char('x'), &rows),
        Some(AgentsOutcome::Ignored)
    );
    assert!(panels.agents_text());
    assert_eq!(
        key(&mut panels, Key::Named(NamedKey::Escape), &rows),
        Some(AgentsOutcome::Blur)
    );
    assert!(panels.open && !panels.focused);
    assert!(!panels.agents_text());
}

#[test]
fn the_files_keys_and_text_stay_out_while_agents_are_shown() {
    let mut panels = opened();
    let keys = KeysConfig::default();
    assert_eq!(
        panels.key(&press(Key::Char('j')), &keys, false, Instant::now()),
        None
    );
    assert_eq!(panels.text("x"), None);
    panels.explorer_command();
    assert!(!panels.agents_text());
    assert_eq!(key(&mut panels, Key::Char('j'), &agents()), None);
}

#[test]
fn the_prefix_still_reaches_the_server_while_agents_are_focused() {
    let rows = agents();
    let mut panels = opened();
    let keys = KeysConfig::default();
    let now = Instant::now();
    let ctrl_a = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..press(Key::Char('a'))
    };
    assert_eq!(panels.agents_key(&ctrl_a, &keys, false, now, &rows), None);
    assert_eq!(
        panels.agents_key(&press(Key::Char('a')), &keys, false, now, &rows),
        None
    );
    assert_eq!(
        panels.agents_key(&press(Key::Char('j')), &keys, true, now, &rows),
        None
    );
}

#[test]
fn a_click_moves_the_cursor_and_jumps() {
    let rows = agents();
    let mut panels = opened();
    assert_eq!(
        panels.agent_click(PaneId(2), &rows),
        AgentsOutcome::Jump(WindowId(12), PaneId(2))
    );
    assert_eq!(panels.agent, Some(PaneId(2)));
    assert!(!panels.focused);
    assert_eq!(panels.agent_click(PaneId(9), &rows), AgentsOutcome::Ignored);
}

#[test]
fn the_files_button_switches_from_agents_to_files_without_closing() {
    let mut panels = opened();
    assert!(panels.toggle_explorer().is_empty());
    assert!(panels.open && panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
    assert!(panels.toggle_explorer().is_empty());
    assert!(!panels.open && !panels.focused);
}

#[test]
fn a_switcher_click_to_agents_starts_the_cursor_like_the_command() {
    let mut panels = Panels::default();
    panels.explorer_command();
    panels.agent = Some(PaneId(9));
    panels.switch(PanelView::Agents, &agents());
    assert!(panels.focused);
    assert_eq!(panels.shown, PanelView::Agents);
    assert_eq!(panels.agent, Some(PaneId(3)));
    assert_eq!(
        key(&mut panels, Key::Named(NamedKey::Enter), &agents()),
        Some(AgentsOutcome::Jump(WindowId(13), PaneId(3)))
    );
    panels.switch(PanelView::Explorer, &agents());
    assert!(panels.focused);
    assert_eq!(panels.shown, PanelView::Explorer);
    panels.switch(PanelView::Tabs, &agents());
    assert!(!panels.focused);
    assert_eq!(panels.shown, PanelView::Tabs);
}
