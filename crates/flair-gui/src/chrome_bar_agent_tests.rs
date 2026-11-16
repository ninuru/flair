use super::*;

fn with_agent(icons: Option<IconSide>, agent: Option<(AgentState, bool)>) -> StatusParts {
    let mut status = parts();
    status.tab_icon = icons;
    status.tabs[0].icon = Some('\u{f120}');
    status.tabs[1].icon = Some('\u{f308}');
    status.tabs[1].agent = agent;
    status
}

fn icon_colors(layout: &ChromeLayout) -> Vec<Rgb> {
    layout
        .chrome
        .text
        .iter()
        .filter(|run| run.face == TextFace::PromptIcon)
        .map(|run| run.color)
        .collect()
}

fn dots(layout: &ChromeLayout) -> Vec<&Quad> {
    layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect.w == DOT && quad.rect.h == DOT && quad.radius == DOT / 2.0)
        .collect()
}

#[test]
fn a_window_agent_tints_its_pill_icon_by_state() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    for (agent, color) in [
        (None, surfaces.muted),
        (Some((AgentState::Working, false)), theme.accent),
        (Some((AgentState::Blocked, false)), surfaces.attention),
        (Some((AgentState::Error, false)), theme.normal[1]),
        (Some((AgentState::Done, true)), theme.foreground),
        (Some((AgentState::Done, false)), surfaces.muted),
        (Some((AgentState::Idle, true)), surfaces.muted),
    ] {
        let status = with_agent(Some(IconSide::After), agent);
        let layout = Case::new(&status).run();
        assert_eq!(icon_colors(&layout), [surfaces.bright, color], "{agent:?}");
    }
}

#[test]
fn states_that_need_you_put_a_dot_on_the_icon_corner() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    for (agent, color) in [
        (Some((AgentState::Blocked, false)), Some(surfaces.attention)),
        (Some((AgentState::Error, true)), Some(theme.normal[1])),
        (Some((AgentState::Done, true)), Some(theme.foreground)),
        (Some((AgentState::Done, false)), None),
        (Some((AgentState::Working, true)), None),
        (Some((AgentState::Idle, false)), None),
        (None, None),
    ] {
        let status = with_agent(Some(IconSide::After), agent);
        let layout = Case::new(&status).run();
        let found: Vec<_> = dots(&layout)
            .iter()
            .map(|quad| (quad.rect, quad.color))
            .collect();
        let expected: Vec<_> = color
            .map(|color| (rect(227.0, 5.0, DOT, DOT), opaque(color)))
            .into_iter()
            .collect();
        assert_eq!(found, expected, "{agent:?}");
        assert_eq!(layout.hits[2].rect, rect(149.0, 0.0, 89.0, 34.0));
    }
}

#[test]
fn the_dot_follows_a_leading_icon_without_widening_the_pill() {
    let status = with_agent(Some(IconSide::Before), Some((AgentState::Blocked, false)));
    let layout = Case::new(&status).run();
    let editor = text(&layout, "editor").unwrap();
    let dot = dots(&layout)[0].rect;
    assert_eq!(dot, rect(171.0 + 11.0 - 3.0, 5.0, DOT, DOT));
    assert!(dot.x + dot.w <= editor.x);
    assert_eq!(layout.hits[2].rect, rect(149.0, 0.0, 89.0, 34.0));
}

#[test]
fn without_icons_the_dot_trails_the_name_and_widens_the_pill() {
    let theme = Config::default().theme();
    for status in [
        with_agent(None, Some((AgentState::Error, false))),
        StatusParts {
            tabs: parts()
                .tabs
                .into_iter()
                .map(|tab| TabParts {
                    agent: (tab.index == 2).then_some((AgentState::Error, false)),
                    ..tab
                })
                .collect(),
            ..parts()
        },
    ] {
        let layout = Case::new(&status).run();
        let editor = text(&layout, "editor").unwrap();
        assert_eq!(editor.x, 154.0);
        let found: Vec<_> = dots(&layout)
            .iter()
            .map(|quad| (quad.rect, quad.color))
            .collect();
        assert_eq!(
            found,
            [(
                rect(154.0 + 42.0 + 6.0, 10.0, DOT, DOT),
                opaque(theme.normal[1])
            )]
        );
        assert_eq!(layout.hits[2].rect, rect(132.0, 0.0, 72.0 + 12.0, 34.0));
    }
}

#[test]
fn working_without_icons_draws_nothing_extra() {
    let status = with_agent(None, Some((AgentState::Working, false)));
    let layout = Case::new(&status).run();
    assert!(dots(&layout).is_empty());
    assert!(icon_colors(&layout).is_empty());
    assert_eq!(layout.hits[2].rect, rect(132.0, 0.0, 72.0, 34.0));
}
