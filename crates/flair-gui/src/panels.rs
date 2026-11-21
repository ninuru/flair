use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use flair_core::{
    agent::AgentState,
    command::{Command, PaneRef},
    config::{Config, EXPLORER_WIDTH, Side, TABS_WIDTH, TabsPosition},
    geom::Direction,
    ids::{PaneId, WindowId},
    key::{InputEvent, Key, KeyAction, KeyInput, NamedKey},
    keys::{KeyChord, KeysConfig},
    proto::{ClientMsg, WindowLayout},
    status::ActivePane,
};

use crate::{
    agents_layout::{Line, lines},
    chrome_layout::{Dock, Frame, PanelView},
    explorer::{Explorer, ExplorerOutcome, quote},
    notice::Notice,
    panel_layout::tab_rows,
    status::AgentParts,
    ui::{DRAG_START, px},
};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pages {
    pub tabs: usize,
    pub explorer: usize,
    pub agents: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    Tab {
        window: WindowId,
        from: f32,
        moved: bool,
    },
    Edge {
        side: Side,
        from: f32,
        width: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerAct {
    Ignored,
    Redraw,
    Load(Vec<PathBuf>),
    Send(ClientMsg),
    Copy(String),
    Notice(Notice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsOutcome {
    Ignored,
    Redraw,
    Jump(WindowId, PaneId),
    Blur,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Panels {
    pub flipped: Option<TabsPosition>,
    pub shown: PanelView,
    pub tabs_scroll: usize,
    pub drag: Option<Drag>,
    pub live_width: Option<(PanelView, f32)>,
    pub explorer: Explorer,
    pub open: bool,
    pub focused: bool,
    pub agent: Option<PaneId>,
    pub agents_scroll: usize,
    pages: Pages,
    reveal: Option<usize>,
    agent_reveal: Option<usize>,
    pane: Option<ActivePane>,
    hidden: Option<bool>,
    editor: String,
    armed: Option<Instant>,
    last_click: Option<(PathBuf, Instant)>,
}

impl Panels {
    pub fn dock(&self, config: &Config, zen: bool) -> Dock {
        let width = |view, configured: u16| match self.live_width {
            Some((live, width)) if live == view => width,
            Some(_) | None => f32::from(configured),
        };
        let flipped = self.flipped == Some(config.tabs.position);
        let side = match (config.tabs.position, flipped) {
            (TabsPosition::Top, false) | (TabsPosition::Left | TabsPosition::Right, true) => None,
            (TabsPosition::Top | TabsPosition::Left, _) => Some(Side::Left),
            (TabsPosition::Right, false) => Some(Side::Right),
        };
        Dock {
            tabs: side
                .filter(|_| !zen)
                .map(|side| (side, width(PanelView::Tabs, config.tabs.width))),
            explorer: (self.open && !zen).then(|| {
                (
                    config.explorer.side,
                    width(PanelView::Explorer, config.explorer.width),
                )
            }),
            shown: self.shown,
        }
    }

    pub fn follow_position(&mut self, position: TabsPosition) {
        if self.flipped.is_some_and(|flipped| flipped != position) {
            self.flipped = None;
        }
    }

    pub fn toggle_tabs(&mut self, position: TabsPosition) {
        self.flipped = match self.flipped == Some(position) {
            true => None,
            false => Some(position),
        };
        self.drag = None;
        self.live_width = None;
        self.pages = Pages::default();
    }

    pub fn place(&mut self, pages: Pages) -> bool {
        self.pages = pages;
        let page = match self.shown {
            PanelView::Agents => pages.agents,
            PanelView::Tabs | PanelView::Explorer => pages.explorer,
        };
        if page == 0 {
            self.focused = false;
        }
        let before = (self.tabs_scroll, self.explorer.scroll(), self.agents_scroll);
        if let Some(active) = self.reveal.take() {
            self.reveal_tab(active);
        }
        if let Some(line) = self.agent_reveal.take() {
            self.reveal_line(line);
        }
        self.explorer.set_page(pages.explorer);
        (self.tabs_scroll, self.explorer.scroll(), self.agents_scroll) != before
    }

    pub fn reveal_tab(&mut self, active: usize) {
        let page = self.pages.tabs;
        if page == 0 {
            self.reveal = Some(active);
            return;
        }
        if active < self.tabs_scroll {
            self.tabs_scroll = active;
        } else if active >= self.tabs_scroll + page {
            self.tabs_scroll = active + 1 - page;
        }
    }

    pub fn scroll_tabs(&mut self, rows: i32, count: usize) {
        let last = (count + 1).saturating_sub(self.pages.tabs);
        let next = self.tabs_scroll.min(last) as i64 + i64::from(rows);
        self.tabs_scroll = next.clamp(0, last as i64) as usize;
    }

    pub fn drop_index(&self, frame: &Frame, y: f32, count: usize, scale: f32) -> u16 {
        let Some(slot) = frame.slots().find(|slot| slot.view == PanelView::Tabs) else {
            return 1;
        };
        let rows = tab_rows(slot, scale);
        let first = self.tabs_scroll.min((count + 1).saturating_sub(rows.page));
        let row = ((y - rows.top) / rows.pitch).floor().max(0.0) as usize + first;
        u16::try_from(row.min(count.saturating_sub(1)) + 1).unwrap_or(u16::MAX)
    }

    pub fn drag_tab(&mut self, y: f32, scale: f32) -> bool {
        match &mut self.drag {
            Some(Drag::Tab { from, moved, .. }) if !*moved => {
                *moved = (y - *from).abs() > px(DRAG_START, scale);
                *moved
            }
            Some(Drag::Tab { .. } | Drag::Edge { .. }) | None => false,
        }
    }

    pub fn drag_edge(&mut self, view: PanelView, x: f32, limit: f32, scale: f32) -> bool {
        let Some(Drag::Edge { side, from, width }) = self.drag else {
            return false;
        };
        let (min, max) = bounds(view);
        if limit < min {
            return self.live_width.take().is_some();
        }
        let delta = (x - from) / scale;
        let wanted = match side {
            Side::Left => width + delta,
            Side::Right => width - delta,
        };
        let next = wanted.round().min(max.min(limit)).max(min);
        let changed = self.live_width != Some((view, next));
        self.live_width = Some((view, next));
        changed
    }
}

impl Panels {
    pub fn explorer_command(&mut self) -> Vec<PathBuf> {
        match (self.open, self.focused && self.shown == PanelView::Explorer) {
            (true, true) => {
                self.close();
                Vec::new()
            }
            (true, false) => {
                self.focused = true;
                self.shown = PanelView::Explorer;
                Vec::new()
            }
            (false, _) => self.open_explorer(),
        }
    }

    pub fn follow(&mut self, pane: Option<ActivePane>) -> Vec<PathBuf> {
        let cwd = pane.as_ref().and_then(|pane| pane.cwd.clone());
        self.pane = pane;
        match cwd {
            Some(cwd) if self.open && self.explorer.root() != Some(cwd.as_path()) => {
                self.explorer.set_root(&cwd)
            }
            Some(_) | None => Vec::new(),
        }
    }

    pub fn scroll_explorer(&mut self, rows: i32) {
        self.explorer.wheel(rows, self.pages.explorer);
    }

    pub fn show_hidden(&mut self, show: bool) {
        if self.hidden != Some(show) {
            self.hidden = Some(show);
            self.explorer.set_show_hidden(show);
        }
    }

    pub fn editor(&self) -> &str {
        &self.editor
    }

    pub fn set_editor(&mut self, editor: String) {
        self.editor = editor;
    }

    pub fn at_prompt(&self) -> bool {
        self.pane.as_ref().is_some_and(|pane| pane.at_prompt)
    }

    pub fn key(
        &mut self,
        key: &KeyInput,
        keys: &KeysConfig,
        pending: bool,
        now: Instant,
    ) -> Option<ExplorerOutcome> {
        if !self.files_focused() || !self.passes(key, keys, pending, now) {
            return None;
        }
        Some(
            self.explorer
                .key(key, self.at_prompt(), self.pages.explorer),
        )
    }

    pub fn has_focus(&self) -> bool {
        self.files_focused() || self.agents_focused()
    }

    pub fn nav(
        &mut self,
        key: &KeyInput,
        keys: &KeysConfig,
        pending: bool,
        now: Instant,
        frame: &Frame,
        layout: &WindowLayout,
    ) -> bool {
        if pending
            || !keys.enabled
            || key.action == KeyAction::Release
            || self.armed.is_some_and(|until| now < until)
        {
            return false;
        }
        let Some(slot) = frame.slots().find(|slot| match slot.view {
            PanelView::Explorer | PanelView::Agents => true,
            PanelView::Tabs => false,
        }) else {
            return false;
        };
        let Some(Command::SelectPane(PaneRef::Direction(direction))) = keys.binding(key, false)
        else {
            return false;
        };
        let (toward, away) = match slot.side {
            Side::Left => (Direction::Left, Direction::Right),
            Side::Right => (Direction::Right, Direction::Left),
        };
        if self.focused && direction == away {
            self.focused = false;
            true
        } else if !self.focused && direction == toward && at_edge(layout, slot.side) {
            self.focused = true;
            true
        } else {
            false
        }
    }

    fn files_focused(&self) -> bool {
        self.open && self.focused && self.shown == PanelView::Explorer
    }

    fn passes(&mut self, key: &KeyInput, keys: &KeysConfig, pending: bool, now: Instant) -> bool {
        if pending || self.armed.is_some_and(|until| now < until) {
            if key.action == KeyAction::Press && !modifier(key.key) {
                self.armed = None;
            }
            return false;
        }
        self.armed = None;
        let prefix = keys.enabled
            && keys
                .key
                .parse::<KeyChord>()
                .is_ok_and(|chord| chord.matches(key));
        if prefix {
            if key.action == KeyAction::Press {
                self.armed = Some(now + Duration::from_millis(keys.timeout_ms));
            }
            return false;
        }
        !(keys.enabled && keys.binding(key, false).is_some())
    }

    pub fn text(&mut self, text: &str) -> Option<ExplorerOutcome> {
        self.files_focused().then(|| self.explorer.text(text))
    }

    pub fn click(&mut self, row: usize, now: Instant) -> ExplorerOutcome {
        let path = self.explorer.rows().get(row).map(|row| row.path.clone());
        let double = self.last_click.as_ref().is_some_and(|(at, when)| {
            path.as_ref() == Some(at) && now.duration_since(*when) < DOUBLE_CLICK
        });
        self.last_click = path.filter(|_| !double).map(|path| (path, now));
        self.focused = true;
        self.explorer.click(row, double)
    }

    pub fn act(&mut self, outcome: ExplorerOutcome) -> ExplorerAct {
        match outcome {
            ExplorerOutcome::Ignored => ExplorerAct::Ignored,
            ExplorerOutcome::Redraw => ExplorerAct::Redraw,
            ExplorerOutcome::Load(dirs) => ExplorerAct::Load(dirs),
            ExplorerOutcome::Insert(path) => match plain(&path) {
                Ok(text) => ExplorerAct::Send(ClientMsg::Input(InputEvent::Paste(format!(
                    "{} ",
                    quote(text)
                )))),
                Err(notice) => ExplorerAct::Notice(notice),
            },
            ExplorerOutcome::Cd(path) if self.at_prompt() => match plain(&path) {
                Ok(text) => ExplorerAct::Send(ClientMsg::Input(InputEvent::Text(format!(
                    "\u{5}\u{15}cd -- {}\r",
                    quote(text)
                )))),
                Err(notice) => ExplorerAct::Notice(notice),
            },
            ExplorerOutcome::Cd(_) => ExplorerAct::Ignored,
            ExplorerOutcome::Edit(path) => self.edit(&path),
            ExplorerOutcome::Copy(path) => match plain(&path) {
                Ok(text) => ExplorerAct::Copy(text.to_owned()),
                Err(notice) => ExplorerAct::Notice(notice),
            },
            ExplorerOutcome::Blur => {
                self.focused = false;
                ExplorerAct::Redraw
            }
        }
    }

    fn edit(&mut self, path: &Path) -> ExplorerAct {
        let Some(root) = self.explorer.root() else {
            return ExplorerAct::Ignored;
        };
        let (file, cwd) = match (plain(path), plain(root)) {
            (Ok(file), Ok(cwd)) => (file.to_owned(), PathBuf::from(cwd)),
            (Err(notice), _) | (_, Err(notice)) => return ExplorerAct::Notice(notice),
        };
        if !cwd.is_dir() || !path.exists() {
            return ExplorerAct::Notice(Notice::error(format!("{file} is gone")));
        }
        self.focused = false;
        let argv = match self.editor.trim() {
            "" => vec![
                "/bin/sh".into(),
                "-c".into(),
                "exec ${EDITOR:-vi} \"$1\"".into(),
                "flair-edit".into(),
                file,
            ],
            editor => vec![
                "/bin/sh".into(),
                "-c".into(),
                "exec $1 \"$2\"".into(),
                "flair-edit".into(),
                editor.to_owned(),
                file,
            ],
        };
        ExplorerAct::Send(ClientMsg::Command(Command::SplitRun { argv, cwd }))
    }

    fn open_explorer(&mut self) -> Vec<PathBuf> {
        self.open = true;
        self.focused = true;
        self.shown = PanelView::Explorer;
        let cwd = self.pane.as_ref().and_then(|pane| pane.cwd.clone());
        match cwd {
            Some(cwd) if self.explorer.root() != Some(cwd.as_path()) => {
                self.explorer.set_root(&cwd)
            }
            Some(_) | None => self.explorer.reload(),
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.focused = false;
        self.armed = None;
        if let Some((PanelView::Explorer | PanelView::Agents, _)) = self.live_width {
            self.live_width = None;
        }
    }
}

impl Panels {
    pub fn agents_command(&mut self, agents: &[AgentParts]) -> Vec<PathBuf> {
        match (self.open, self.shown == PanelView::Agents) {
            (true, true) => {
                self.close();
                Vec::new()
            }
            (true, false) => {
                let focused = self.focused;
                self.show_agents(agents);
                self.focused = focused;
                Vec::new()
            }
            (false, _) => {
                let dirs = self.open_explorer();
                self.show_agents(agents);
                self.focused = false;
                dirs
            }
        }
    }

    pub fn switch(&mut self, view: PanelView, agents: &[AgentParts]) {
        match view {
            PanelView::Agents => {
                self.show_agents(agents);
                self.focused = self.open;
            }
            PanelView::Explorer => {
                self.shown = view;
                self.focused = self.open;
            }
            PanelView::Tabs => {
                self.shown = view;
                self.focused = false;
            }
        }
    }

    pub fn agents_key(
        &mut self,
        key: &KeyInput,
        keys: &KeysConfig,
        pending: bool,
        now: Instant,
        agents: &[AgentParts],
    ) -> Option<AgentsOutcome> {
        if !self.agents_focused() || !self.passes(key, keys, pending, now) {
            return None;
        }
        if key.action == KeyAction::Release {
            return Some(AgentsOutcome::Ignored);
        }
        let outcome = match key.key {
            Key::Named(NamedKey::Up) | Key::Char('k') => self.step_agent(-1, agents),
            Key::Named(NamedKey::Down) | Key::Char('j') => self.step_agent(1, agents),
            Key::Named(NamedKey::Enter) => match self.cursor(agents) {
                Some(agent) => {
                    self.focused = false;
                    AgentsOutcome::Jump(agent.window, agent.pane)
                }
                None => AgentsOutcome::Ignored,
            },
            Key::Named(NamedKey::Escape) => {
                self.focused = false;
                AgentsOutcome::Blur
            }
            Key::Char(_) | Key::Named(_) => AgentsOutcome::Ignored,
        };
        Some(outcome)
    }

    pub fn agents_text(&self) -> bool {
        self.agents_focused()
    }

    pub fn agent_click(&mut self, pane: PaneId, agents: &[AgentParts]) -> AgentsOutcome {
        match agents.iter().find(|agent| agent.pane == pane) {
            Some(agent) => {
                self.agent = Some(pane);
                self.focused = false;
                AgentsOutcome::Jump(agent.window, agent.pane)
            }
            None => AgentsOutcome::Ignored,
        }
    }

    pub fn scroll_agents(&mut self, rows: i32, lines: usize) {
        let last = lines.saturating_sub(self.pages.agents);
        let next = self.agents_scroll.min(last) as i64 + i64::from(rows);
        self.agents_scroll = next.clamp(0, last as i64) as usize;
    }

    fn agents_focused(&self) -> bool {
        self.open && self.focused && self.shown == PanelView::Agents
    }

    fn show_agents(&mut self, agents: &[AgentParts]) {
        self.focused = true;
        self.shown = PanelView::Agents;
        self.agent = start(agents).map(|agent| agent.pane);
        self.reveal_agent(agents);
    }

    fn cursor<'a>(&self, agents: &'a [AgentParts]) -> Option<&'a AgentParts> {
        agents
            .iter()
            .find(|agent| Some(agent.pane) == self.agent)
            .or_else(|| start(agents))
    }

    fn step_agent(&mut self, delta: isize, agents: &[AgentParts]) -> AgentsOutcome {
        let Some(current) = self.cursor(agents) else {
            return AgentsOutcome::Ignored;
        };
        let index = agents
            .iter()
            .position(|agent| agent.pane == current.pane)
            .unwrap_or(0);
        let next = index
            .saturating_add_signed(delta)
            .min(agents.len().saturating_sub(1));
        self.agent = Some(agents[next].pane);
        self.reveal_agent(agents);
        AgentsOutcome::Redraw
    }

    fn reveal_agent(&mut self, agents: &[AgentParts]) {
        let line = lines(agents).iter().position(|line| match line {
            Line::Agent(agent) => Some(agent.pane) == self.agent,
            Line::Session(_) => false,
        });
        if let Some(line) = line {
            self.reveal_line(line);
        }
    }

    fn reveal_line(&mut self, line: usize) {
        let page = self.pages.agents;
        if page == 0 {
            self.agent_reveal = Some(line);
            return;
        }
        if line < self.agents_scroll {
            self.agents_scroll = line;
        } else if line >= self.agents_scroll + page {
            self.agents_scroll = line + 1 - page;
        }
    }
}

fn start(agents: &[AgentParts]) -> Option<&AgentParts> {
    agents
        .iter()
        .find(|agent| agent.state == AgentState::Blocked)
        .or_else(|| agents.iter().find(|agent| agent.unseen))
        .or_else(|| agents.first())
}

fn at_edge(layout: &WindowLayout, side: Side) -> bool {
    let area = layout.area;
    layout
        .panes
        .iter()
        .find(|placement| placement.pane == layout.active)
        .is_some_and(|placement| {
            let rect = placement.rect;
            match side {
                Side::Left => rect.origin.col <= area.origin.col,
                Side::Right => rect.origin.col + rect.size.cols >= area.origin.col + area.size.cols,
            }
        })
}

fn plain(path: &Path) -> Result<&str, Notice> {
    let text = path
        .to_str()
        .ok_or_else(|| Notice::error("path is not valid UTF-8"))?;
    match text.chars().any(char::is_control) {
        true => Err(Notice::error("path has control characters")),
        false => Ok(text),
    }
}

fn modifier(key: Key) -> bool {
    matches!(
        key,
        Key::Named(
            NamedKey::ShiftLeft
                | NamedKey::ShiftRight
                | NamedKey::ControlLeft
                | NamedKey::ControlRight
                | NamedKey::AltLeft
                | NamedKey::AltRight
                | NamedKey::SuperLeft
                | NamedKey::SuperRight
        )
    )
}

pub fn bounds(view: PanelView) -> (f32, f32) {
    let (min, max) = match view {
        PanelView::Tabs => TABS_WIDTH,
        PanelView::Explorer | PanelView::Agents => EXPLORER_WIDTH,
    };
    (f32::from(min), f32::from(max))
}

#[cfg(test)]
#[path = "panels_tests.rs"]
mod tests;
