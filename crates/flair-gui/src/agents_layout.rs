use flair_core::{
    agent::AgentState,
    theme::{Rgb, Theme},
};

use crate::{
    chrome_bar::{
        Scene, baseline, cap_centre, ellipsize, fill, icon_text, rounded, small_text, ui_text,
    },
    chrome_layout::{ChromeInput, Hit, HitTarget, Slot},
    overlay_layout::inside,
    panel_layout::{content, icon_column, tab_rows},
    renderer::PxRect,
    status::AgentParts,
    ui::{INSET, ITEM_GAP, ROW_PAD, ROW_RADIUS, SELECT_BAR, SELECT_INSET, px},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Line<'a> {
    Session(&'a str),
    Agent(&'a AgentParts),
}

pub(crate) fn lines(agents: &[AgentParts]) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut session = None;
    for agent in agents {
        if session != Some(agent.session.as_str()) {
            session = Some(agent.session.as_str());
            lines.push(Line::Session(&agent.session));
        }
        lines.push(Line::Agent(agent));
    }
    lines
}

pub(crate) fn draw(
    input: &ChromeInput,
    slot: Slot,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) -> usize {
    let Some(panels) = input.panels else {
        return 0;
    };
    let scale = input.scale;
    let area = content(slot, scale);
    let rows = tab_rows(slot, scale);
    let inset = px(INSET, scale);
    let row_rect = |slot: usize| PxRect {
        x: area.x + inset,
        y: rows.top + slot as f32 * rows.pitch,
        w: (area.w - inset * 2.0).max(0.0),
        h: rows.height,
    };
    let lines = lines(&input.status.agents);
    if lines.is_empty() && rows.page > 0 {
        let rect = row_rect(0);
        scene.text.push(ui_text(
            (rect.x + px(ROW_PAD, scale)).round(),
            baseline(rect, input.ui),
            "no agents".to_owned(),
            input.theme.surfaces().faint,
            false,
        ));
    }
    let first = panels
        .agents_scroll
        .min(lines.len().saturating_sub(rows.page));
    for (slot, line) in lines.iter().skip(first).take(rows.page).enumerate() {
        let rect = row_rect(slot);
        match line {
            Line::Session(name) => scene.text.push(small_text(
                (rect.x + px(ROW_PAD, scale)).round(),
                rect,
                (*name).to_owned(),
                input.theme.surfaces().faint,
                input.ui,
            )),
            Line::Agent(agent) => {
                let state = RowState {
                    cursor: panels.focused && panels.agent == Some(agent.pane),
                    hovered: input.hover.is_some_and(|point| inside(rect, point)),
                };
                agent_row(input, agent, rect, state, scene, measure);
            }
        }
    }
    rows.page
}

#[derive(Clone, Copy)]
struct RowState {
    cursor: bool,
    hovered: bool,
}

fn agent_row(
    input: &ChromeInput,
    agent: &AgentParts,
    rect: PxRect,
    state: RowState,
    scene: &mut Scene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let scale = input.scale;
    let theme = input.theme;
    let surfaces = theme.surfaces();
    if agent.active {
        scene
            .quads
            .push(rounded(rect, surfaces.accent_soft, ROW_RADIUS, scale));
        let inset = px(SELECT_INSET, scale);
        scene.quads.push(fill(
            PxRect {
                x: rect.x,
                y: rect.y + inset,
                w: px(SELECT_BAR, scale),
                h: (rect.h - inset * 2.0).max(0.0),
            },
            theme.accent,
        ));
    } else if state.cursor {
        scene
            .quads
            .push(rounded(rect, surfaces.surface3, ROW_RADIUS, scale));
    } else if state.hovered {
        scene
            .quads
            .push(rounded(rect, surfaces.surface2, ROW_RADIUS, scale));
    }
    let mut x = rect.x + px(ROW_PAD, scale);
    if let Some(icon) = agent.icon {
        let color = match agent.active {
            true => surfaces.bright,
            false => surfaces.muted,
        };
        scene.text.push(icon_text(
            x,
            cap_centre(rect, input.ui),
            icon,
            color,
            input.metrics,
        ));
    }
    x += icon_column(input);
    let label_width = measure(agent.label, false) * input.ui.small;
    let label_x = (rect.x + rect.w - px(ROW_PAD, scale) - label_width).round();
    let label_color = alert(theme, agent.state, agent.unseen).unwrap_or(match agent.state {
        AgentState::Idle => surfaces.faint,
        AgentState::Blocked | AgentState::Error | AgentState::Done | AgentState::Working => {
            surfaces.muted
        }
    });
    scene.text.push(small_text(
        label_x,
        rect,
        agent.label.to_owned(),
        label_color,
        input.ui,
    ));
    let title = ellipsize(
        &agent.title,
        label_x - px(ITEM_GAP, scale) - x,
        false,
        measure,
    );
    let color = if agent.unseen || agent.active {
        theme.foreground
    } else if state.hovered {
        surfaces.bright
    } else {
        surfaces.muted
    };
    if !title.is_empty() {
        scene.text.push(ui_text(
            x.round(),
            baseline(rect, input.ui),
            title,
            color,
            false,
        ));
    }
    scene.hits.push(Hit {
        rect,
        target: HitTarget::Agent(agent.pane),
    });
}

pub(crate) fn alert(theme: &Theme, state: AgentState, unseen: bool) -> Option<Rgb> {
    match state {
        AgentState::Blocked => Some(theme.surfaces().attention),
        AgentState::Error => Some(theme.normal[1]),
        AgentState::Done if unseen => Some(theme.foreground),
        AgentState::Done | AgentState::Working | AgentState::Idle => None,
    }
}

#[cfg(test)]
#[path = "agents_layout_tests.rs"]
mod tests;
