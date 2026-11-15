use super::*;
use crate::{chrome_layout::PanelView, status::AgentParts, ui::BELL_KEY};
use flair_core::agent::AgentState;

fn glyph(layout: &ChromeLayout) -> Option<&TextRun> {
    text(layout, &FILES_KEY.to_string())
}

#[test]
fn the_explorer_button_sits_at_the_far_right_of_the_bar() {
    let surfaces = Config::default().theme().surfaces();
    let status = parts();
    let layout = Case::new(&status).run();
    let button = rect(768.0, 0.0, 24.0, 34.0);
    assert_eq!(
        hit_test(&layout.hits, (780.0, 3.0)),
        Some(HitTarget::ExplorerButton)
    );
    assert!(layout.hits.iter().any(|hit| hit.rect == button));
    let run = glyph(&layout).unwrap();
    assert_eq!(run.face, TextFace::PromptIcon);
    assert_eq!(run.color, surfaces.muted);
    assert!(run.x >= 768.0 && run.x < 780.0, "{}", run.x);
    assert!(quad_at(&layout, rect(768.0, 5.0, 24.0, 24.0)).is_none());
}

#[test]
fn hovering_the_button_fills_it_and_an_open_explorer_brightens_it() {
    let theme = Config::default().theme();
    let status = parts();
    let hovered = Case {
        hover: Some((780.0, 10.0)),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        quad_at(&hovered, rect(768.0, 5.0, 24.0, 24.0)),
        Some(&rounded(
            rect(768.0, 5.0, 24.0, 24.0),
            theme.surfaces().surface2,
            ROW_RADIUS,
            1.0
        ))
    );
    let mut open = Panels::default();
    open.open = true;
    let layout = Case {
        panels: Some(&open),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(glyph(&layout).unwrap().color, theme.foreground);
}

#[test]
fn the_badges_move_one_item_left_of_the_button() {
    let status = copy_and_prefix();
    let layout = Case::new(&status).run();
    let copy = quad_at(&layout, rect(736.0, 7.0, 26.0, 20.0)).unwrap();
    assert_eq!(copy.rect.x + copy.rect.w + 6.0, 768.0);
}

fn with_agents(states: &[(AgentState, bool)]) -> StatusParts {
    StatusParts {
        agents: states
            .iter()
            .enumerate()
            .map(|(index, (state, unseen))| AgentParts {
                pane: PaneId(index as u32 + 1),
                window: WindowId(1),
                session: "main".into(),
                icon: None,
                title: "agent".into(),
                label: state.label(None),
                state: *state,
                unseen: *unseen,
                active: false,
            })
            .collect(),
        ..copy_and_prefix()
    }
}

fn bells(layout: &ChromeLayout) -> Vec<&TextRun> {
    layout
        .chrome
        .text
        .iter()
        .filter(|run| run.text == BELL_KEY.to_string() && run.baseline < 34.0)
        .collect()
}

#[test]
fn the_agents_indicator_shows_only_for_unseen_rows() {
    let status = with_agents(&[(AgentState::Done, false), (AgentState::Working, false)]);
    let layout = Case::new(&status).run();
    assert!(bells(&layout).is_empty());
    assert_eq!(hit_test(&layout.hits, (750.0, 3.0)), None);
    assert!(quad_at(&layout, rect(736.0, 7.0, 26.0, 20.0)).is_some());
}

#[test]
fn an_unseen_row_puts_the_indicator_left_of_the_button_and_shifts_the_badges() {
    let theme = Config::default().theme();
    let status = with_agents(&[(AgentState::Working, false), (AgentState::Done, true)]);
    let layout = Case::new(&status).run();
    let bell = bells(&layout);
    assert_eq!(bell.len(), 1);
    assert_eq!(bell[0].color, theme.foreground);
    assert!(bell[0].x >= 738.0 && bell[0].x < 750.0, "{}", bell[0].x);
    assert_eq!(
        hit_test(&layout.hits, (750.0, 3.0)),
        Some(HitTarget::AgentsButton)
    );
    assert!(
        layout
            .hits
            .iter()
            .any(|hit| hit.rect == rect(738.0, 0.0, 24.0, 34.0))
    );
    assert!(quad_at(&layout, rect(706.0, 7.0, 26.0, 20.0)).is_some());
    let hovered = Case {
        hover: Some((750.0, 10.0)),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        quad_at(&hovered, rect(738.0, 5.0, 24.0, 24.0)),
        Some(&rounded(
            rect(738.0, 5.0, 24.0, 24.0),
            theme.surfaces().surface2,
            ROW_RADIUS,
            1.0
        ))
    );
}

#[test]
fn a_blocked_or_failed_unseen_row_uses_the_attention_colour() {
    let attention = Config::default().theme().surfaces().attention;
    for state in [AgentState::Blocked, AgentState::Error] {
        let status = with_agents(&[(AgentState::Done, true), (state, true)]);
        let layout = Case::new(&status).run();
        assert_eq!(bells(&layout)[0].color, attention, "{state:?}");
    }
}

#[test]
fn the_indicator_hides_while_the_agents_view_is_visible() {
    let status = with_agents(&[(AgentState::Blocked, true)]);
    let mut panels = Panels::default();
    panels.open = true;
    panels.shown = PanelView::Agents;
    let visible = Case {
        panels: Some(&panels),
        dock: panels.dock(&Config::default(), false),
        ..Case::new(&status)
    }
    .run();
    assert!(bells(&visible).is_empty());
    panels.shown = PanelView::Explorer;
    let files = Case {
        panels: Some(&panels),
        dock: panels.dock(&Config::default(), false),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(bells(&files).len(), 1);
}
