use flair_core::{
    agent::{AgentKind, AgentState},
    config::{Config, IconSide, Side},
    ids::{PaneId, WindowId},
    theme::{Rgb, Rgba, Theme},
};

use super::*;
use crate::{
    CellMetrics, UiMetrics,
    chrome_layout::{ChromeLayout, Dock, PanelView, frame, hit_test, layout},
    font::PROMPT_ICON_SCALE,
    panels::Panels,
    renderer::{Quad, TextFace, TextRun},
    status::{AgentParts, ChipParts, StatusParts, TabParts},
    ui::{ELLIPSIS, ITEM_GAP, PROCESS_ICON_GAP},
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

fn metrics() -> CellMetrics {
    CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    }
}

fn ui() -> UiMetrics {
    UiMetrics {
        height: 14.0,
        ascent: 11.0,
        small: 0.5,
        cap: 8.0,
        icon: CellMetrics {
            width: 7.0,
            height: 12.0,
            baseline: 9.0,
        },
    }
}

fn theme() -> Theme {
    Config::default().theme()
}

fn agent(pane: u32, session: &str, title: &str, state: AgentState) -> AgentParts {
    AgentParts {
        pane: PaneId(pane),
        window: WindowId(pane),
        session: session.into(),
        icon: Some('\u{f120}'),
        title: title.into(),
        label: state.label(None),
        state,
        unseen: false,
        active: false,
    }
}

fn three() -> Vec<AgentParts> {
    vec![
        agent(1, "alpha", "Fix login bug", AgentState::Working),
        agent(2, "alpha", "Write docs", AgentState::Done),
        agent(3, "work", "Deploy", AgentState::Idle),
    ]
}

fn status(agents: Vec<AgentParts>) -> StatusParts {
    StatusParts {
        tab_icon: Some(IconSide::After),
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs: vec![TabParts {
            window: WindowId(1),
            index: 1,
            icon: None,
            name: "shell".into(),
            active: true,
            last: false,
            activity: false,
            bell: false,
            agent: None,
        }],
        badges: Vec::new(),
        agents,
    }
}

fn shown() -> Panels {
    let mut panels = Panels::default();
    panels.open = true;
    panels.focused = true;
    panels.shown = PanelView::Agents;
    panels
}

struct Case<'a> {
    status: &'a StatusParts,
    panels: Panels,
    height: u32,
    width: f32,
    hover: Option<(f32, f32)>,
    tabs: Option<Side>,
}

impl<'a> Case<'a> {
    fn new(status: &'a StatusParts) -> Case<'a> {
        Case {
            status,
            panels: shown(),
            height: 600,
            width: 280.0,
            hover: None,
            tabs: None,
        }
    }

    fn run(&self) -> ChromeLayout {
        let theme = theme();
        let px = (1000, self.height);
        let dock = Dock {
            tabs: self.tabs.map(|side| (side, 200.0)),
            explorer: Some((Side::Left, self.width)),
            shown: self.panels.shown,
        };
        let input = ChromeInput {
            px,
            scale: 1.0,
            padding: (8.0, 4.0),
            zen: false,
            metrics: metrics(),
            ui: ui(),
            theme: &theme,
            status: self.status,
            prompt: None,
            layout: None,
            hover: self.hover,
            overlays: None,
            notice: None,
            isles: None,
            frame: frame(px, 1.0, false, (8.0, 4.0), metrics(), dock),
            panels: Some(&self.panels),
        };
        layout(&input, measure)
    }
}

fn run_text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

fn quads_at(layout: &ChromeLayout, area: PxRect) -> Vec<&Quad> {
    layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect == area)
        .collect()
}

const BAND: f32 = 36.0 + 1.0;

fn row(line: usize) -> PxRect {
    rect(8.0, 43.0 + BAND + line as f32 * 32.0, 263.0, 28.0)
}

fn title_x() -> f32 {
    8.0 + 10.0 + (8.0 * PROMPT_ICON_SCALE).round() + PROCESS_ICON_GAP
}

#[test]
fn lines_group_agents_under_their_session() {
    let agents = three();
    let lines = lines(&agents);
    assert_eq!(lines.len(), 5);
    assert!(matches!(lines[0], Line::Session("alpha")));
    assert!(matches!(lines[1], Line::Agent(agent) if agent.pane == PaneId(1)));
    assert!(matches!(lines[2], Line::Agent(agent) if agent.pane == PaneId(2)));
    assert!(matches!(lines[3], Line::Session("work")));
    assert!(matches!(lines[4], Line::Agent(agent) if agent.pane == PaneId(3)));
    assert!(super::lines(&[]).is_empty());
}

#[test]
fn session_headers_are_small_faint_text_without_a_hit() {
    let parts = status(three());
    let layout = Case::new(&parts).run();
    let faint = theme().surfaces().faint;
    let main = run_text(&layout, "alpha").unwrap();
    assert_eq!((main.x, main.baseline), (18.0, 59.0 + BAND));
    assert_eq!(main.face, TextFace::Small);
    assert_eq!(main.color, faint);
    let work = run_text(&layout, "work").unwrap();
    assert_eq!((work.x, work.baseline), (18.0, 155.0 + BAND));
    assert_eq!(
        hit_test(&layout.hits, (40.0, 50.0 + f64::from(BAND))),
        Some(HitTarget::PanelBody)
    );
    let fix = run_text(&layout, "Fix login bug").unwrap();
    assert_eq!((fix.x, fix.baseline), (title_x(), 93.0 + BAND));
    assert_eq!(fix.face, TextFace::Ui);
    assert_eq!(run_text(&layout, "Deploy").unwrap().baseline, 189.0 + BAND);
}

#[test]
fn every_agent_row_is_a_hit_for_its_pane() {
    let parts = status(three());
    let layout = Case::new(&parts).run();
    let at = |line: usize| {
        let rect = row(line);
        hit_test(
            &layout.hits,
            (f64::from(rect.x + 2.0), f64::from(rect.y + 2.0)),
        )
    };
    assert_eq!(at(1), Some(HitTarget::Agent(PaneId(1))));
    assert_eq!(at(2), Some(HitTarget::Agent(PaneId(2))));
    assert_eq!(at(4), Some(HitTarget::Agent(PaneId(3))));
    assert!(layout.hits.iter().any(|hit| hit.rect == row(4)));
}

#[test]
fn labels_are_right_aligned_small_text_coloured_by_state() {
    let theme = theme();
    let surfaces = theme.surfaces();
    let mut done = agent(4, "alpha", "four", AgentState::Done);
    done.unseen = true;
    let mut blocked = agent(5, "alpha", "five", AgentState::Blocked);
    blocked.label = AgentState::Blocked.label(Some(AgentKind::Permission));
    let parts = status(vec![
        blocked,
        agent(6, "alpha", "six", AgentState::Error),
        done,
        agent(7, "alpha", "seven", AgentState::Done),
        agent(8, "alpha", "eight", AgentState::Working),
        agent(9, "alpha", "nine", AgentState::Idle),
    ]);
    let layout = Case::new(&parts).run();
    let label = |line: usize| {
        let rect = row(line);
        layout
            .chrome
            .text
            .iter()
            .find(|run| run.face == TextFace::Small && run.baseline == rect.y + 16.0)
            .unwrap()
    };
    let approval = label(1);
    assert_eq!(approval.text, "Needs approval");
    assert_eq!(approval.x, 8.0 + 263.0 - 10.0 - 49.0);
    assert_eq!(approval.color, surfaces.attention);
    assert_eq!(label(2).text, "Failed");
    assert_eq!(label(2).color, theme.normal[1]);
    assert_eq!(label(3).color, theme.foreground);
    assert_eq!(label(4).color, surfaces.muted);
    assert_eq!(label(5).text, "Working");
    assert_eq!(label(5).color, surfaces.muted);
    assert_eq!(label(6).text, "Idle");
    assert_eq!(label(6).color, surfaces.faint);
}

#[test]
fn unseen_and_active_titles_are_foreground_and_hover_brightens() {
    let theme = theme();
    let surfaces = theme.surfaces();
    let mut agents = three();
    agents[1].unseen = true;
    let parts = status(agents);
    let layout = Case::new(&parts).run();
    assert_eq!(
        run_text(&layout, "Fix login bug").unwrap().color,
        surfaces.muted
    );
    assert_eq!(
        run_text(&layout, "Write docs").unwrap().color,
        theme.foreground
    );
    let hovered = Case {
        hover: Some((40.0, 80.0 + BAND)),
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        run_text(&hovered, "Fix login bug").unwrap().color,
        surfaces.bright
    );
    assert_eq!(
        quads_at(&hovered, row(1)),
        [&rounded(row(1), surfaces.surface2, ROW_RADIUS, 1.0)]
    );
}

#[test]
fn the_active_pane_has_the_soft_accent_fill_and_bar() {
    let theme = theme();
    let surfaces = theme.surfaces();
    let mut agents = three();
    agents[0].active = true;
    let parts = status(agents);
    let layout = Case {
        hover: Some((40.0, 80.0 + BAND)),
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        quads_at(&layout, row(1)),
        [&rounded(row(1), surfaces.accent_soft, ROW_RADIUS, 1.0)]
    );
    let bar = rect(8.0, 75.0 + BAND + 7.0, 2.0, 14.0);
    assert!(
        layout
            .chrome
            .quads
            .iter()
            .any(|quad| quad.rect == bar && quad.color == opaque(theme.accent))
    );
    assert_eq!(
        run_text(&layout, "Fix login bug").unwrap().color,
        theme.foreground
    );
    let icon = layout
        .chrome
        .text
        .iter()
        .find(|run| {
            run.face == TextFace::PromptIcon
                && run.baseline > 35.0 + BAND
                && run.baseline < 107.0 + BAND
        })
        .unwrap();
    assert_eq!(icon.color, surfaces.bright);
    assert_eq!(icon.x, 18.0);
}

#[test]
fn the_keyboard_cursor_is_raised_only_while_focused() {
    let surfaces = theme().surfaces();
    let parts = status(three());
    let mut panels = shown();
    panels.agent = Some(PaneId(2));
    let layout = Case {
        panels: panels.clone(),
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        quads_at(&layout, row(2)),
        [&rounded(row(2), surfaces.surface3, ROW_RADIUS, 1.0)]
    );
    panels.focused = false;
    let blurred = Case {
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert!(quads_at(&blurred, row(2)).is_empty());
}

#[test]
fn a_long_title_ends_in_an_ellipsis_before_the_label() {
    let mut agents = three();
    agents[0].title = "a very long agent title that cannot possibly fit".into();
    let parts = status(agents);
    let layout = Case::new(&parts).run();
    let title = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.ends_with(ELLIPSIS))
        .unwrap();
    let label_x = run_text(&layout, "Working").unwrap().x;
    assert!((236.0..=237.0).contains(&label_x));
    assert!(title.text.starts_with("a very"));
    assert!(title.x + measure(&title.text, false) <= label_x - ITEM_GAP);
}

#[test]
fn rows_that_do_not_fit_are_dropped_whole_and_scroll() {
    let parts = status(three());
    let mut blurred = shown();
    blurred.focused = false;
    let layout = Case {
        height: 35 + 37 + 8 + 32 * 2 + 28 + 8 + 10,
        panels: blurred.clone(),
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(layout.pages.agents, 3);
    assert!(run_text(&layout, "Write docs").is_some());
    assert!(run_text(&layout, "work").is_none());
    assert!(run_text(&layout, "Deploy").is_none());
    let mut panels = blurred;
    panels.agents_scroll = 2;
    let scrolled = Case {
        height: 35 + 37 + 8 + 32 * 2 + 28 + 8 + 10,
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert!(run_text(&scrolled, "alpha").is_none());
    assert_eq!(
        run_text(&scrolled, "Write docs").unwrap().baseline,
        61.0 + BAND
    );
    assert!(run_text(&scrolled, "Deploy").is_some());
}

#[test]
fn no_agents_shows_one_faint_line() {
    let parts = status(Vec::new());
    let layout = Case::new(&parts).run();
    let empty = run_text(&layout, "no agents").unwrap();
    assert_eq!((empty.x, empty.baseline), (18.0, 61.0 + BAND));
    assert_eq!(empty.color, theme().surfaces().faint);
    assert_eq!(empty.face, TextFace::Ui);
}

#[test]
fn a_shared_slot_lists_agents_as_the_third_switcher_item() {
    let theme = theme();
    let parts = status(three());
    let layout = Case {
        tabs: Some(Side::Left),
        ..Case::new(&parts)
    }
    .run();
    let item = run_text(&layout, "Agents").unwrap();
    assert_eq!(item.x, 154.0);
    assert_eq!(item.color, theme.accent);
    assert_eq!(
        run_text(&layout, "Files").unwrap().color,
        theme.surfaces().muted
    );
    assert_eq!(
        hit_test(&layout.hits, (160.0, 50.0)),
        Some(HitTarget::Switch(PanelView::Agents))
    );
    assert!(run_text(&layout, "Fix login bug").is_some());
    let narrow = Case {
        tabs: Some(Side::Left),
        width: 200.0,
        ..Case::new(&parts)
    }
    .run();
    assert!(run_text(&narrow, "Agents").is_none());
    assert!(run_text(&narrow, "Files").is_some());
    assert_eq!(
        hit_test(&narrow.hits, (160.0, 50.0)),
        Some(HitTarget::PanelBody)
    );
}

#[test]
fn an_unshared_agents_slot_has_a_single_item_header_band() {
    let theme = theme();
    let parts = status(three());
    let layout = Case::new(&parts).run();
    let label = run_text(&layout, "Agents").unwrap();
    assert_eq!((label.x, label.baseline), (18.0, 57.0));
    assert_eq!(label.color, theme.accent);
    assert_eq!(label.face, TextFace::Ui);
    assert!(run_text(&layout, "Files").is_none());
    assert!(run_text(&layout, "Windows").is_none());
    assert_eq!(
        quads_at(&layout, rect(16.0, 69.0, 46.0, 2.0)),
        [&fill(rect(16.0, 69.0, 46.0, 2.0), theme.accent)]
    );
    assert_eq!(
        quads_at(&layout, rect(0.0, 71.0, 279.0, 1.0)),
        [&fill(rect(0.0, 71.0, 279.0, 1.0), theme.surfaces().outline)]
    );
    assert_eq!(
        hit_test(&layout.hits, (30.0, 50.0)),
        Some(HitTarget::PanelBody)
    );
    assert_eq!(
        run_text(&layout, "alpha").unwrap().baseline,
        row(0).y + 16.0
    );
}

#[test]
fn the_explorer_slot_has_no_agents_header() {
    let parts = status(three());
    let mut panels = shown();
    panels.shown = PanelView::Explorer;
    let layout = Case {
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert!(run_text(&layout, "Agents").is_none());
    assert!(quads_at(&layout, rect(16.0, 69.0, 46.0, 2.0)).is_empty());
    assert!(quads_at(&layout, rect(0.0, 71.0, 279.0, 1.0)).is_empty());
}

#[test]
fn a_shared_slot_keeps_its_switcher_and_the_same_row_offset() {
    let parts = status(three());
    let shared = Case {
        tabs: Some(Side::Left),
        ..Case::new(&parts)
    }
    .run();
    let unshared = Case::new(&parts).run();
    assert_eq!(
        run_text(&shared, "Windows").unwrap().color,
        theme().surfaces().muted
    );
    assert_eq!(
        hit_test(&shared.hits, (30.0, 50.0)),
        Some(HitTarget::Switch(PanelView::Tabs))
    );
    assert_eq!(
        run_text(&shared, "alpha").unwrap().baseline,
        run_text(&unshared, "alpha").unwrap().baseline
    );
}

#[test]
fn the_header_is_accented_while_focused_and_muted_otherwise() {
    let theme = theme();
    let parts = status(three());
    let underline = rect(16.0, 69.0, 46.0, 2.0);
    let focused = Case::new(&parts).run();
    assert_eq!(run_text(&focused, "Agents").unwrap().color, theme.accent);
    assert_eq!(
        quads_at(&focused, underline),
        [&fill(underline, theme.accent)]
    );
    let mut panels = shown();
    panels.focused = false;
    let blurred = Case {
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        run_text(&blurred, "Agents").unwrap().color,
        theme.foreground
    );
    assert_eq!(
        quads_at(&blurred, underline),
        [&fill(underline, theme.surfaces().faint)]
    );
    assert_ne!(theme.surfaces().faint, theme.accent);
}

#[test]
fn a_shared_switcher_accents_only_the_focused_view() {
    let theme = theme();
    let parts = status(three());
    let focused = Case {
        tabs: Some(Side::Left),
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(run_text(&focused, "Agents").unwrap().color, theme.accent);
    assert_eq!(
        run_text(&focused, "Files").unwrap().color,
        theme.surfaces().muted
    );
    let mut panels = shown();
    panels.focused = false;
    let blurred = Case {
        tabs: Some(Side::Left),
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        run_text(&blurred, "Agents").unwrap().color,
        theme.foreground
    );
}

#[test]
fn the_footer_shows_agent_keys_only_while_focused() {
    let theme = theme();
    let parts = status(three());
    let focused = Case::new(&parts).run();
    for text in [ENTER_KEY, "jump", "j/k", "move", ESCAPE_KEY, "back"] {
        assert!(run_text(&focused, text).is_some(), "{text}");
    }
    assert_eq!(
        quads_at(&focused, rect(0.0, 559.0, 279.0, 1.0)),
        [&fill(
            rect(0.0, 559.0, 279.0, 1.0),
            theme.surfaces().outline
        )]
    );
    let mut panels = shown();
    panels.focused = false;
    let blurred = Case {
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert!(run_text(&blurred, "jump").is_none());
    assert!(quads_at(&blurred, rect(0.0, 559.0, 279.0, 1.0)).is_empty());
    assert!(blurred.pages.agents > focused.pages.agents);
}
