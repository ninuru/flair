use std::path::PathBuf;

use crate::{
    geom::{CellPoint, CellRect, GridSize},
    ids::{PaneId, SessionId, WindowId},
    layout::{Layout, leaf},
    proto::WindowLayout,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub root: PathBuf,
    pub windows: Vec<Window>,
    pub last_window: WindowId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Removed {
    pub window: Option<WindowId>,
    pub session: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: WindowId,
    pub panes: Vec<PaneId>,
    pub active_pane: PaneId,
    pub name: Option<String>,
    pub layout: Layout,
    pub size: GridSize,
    pub zoomed: bool,
}

impl Tree {
    pub fn single(
        session: SessionId,
        name: String,
        root: PathBuf,
        window: WindowId,
        pane: PaneId,
    ) -> Tree {
        Tree {
            sessions: vec![Session {
                id: session,
                name,
                root,
                last_window: window,
                windows: vec![Window::new(window, pane, GridSize { cols: 80, rows: 24 })],
            }],
        }
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn active_pane(&self, window: WindowId) -> Option<PaneId> {
        self.window(window).map(|window| window.active_pane)
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.sessions
            .iter()
            .flat_map(|session| &session.windows)
            .find(|window| window.id == id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.sessions
            .iter_mut()
            .flat_map(|session| &mut session.windows)
            .find(|window| window.id == id)
    }

    pub fn remove_pane(&mut self, pane: PaneId) -> Removed {
        let mut removed = Removed::default();
        for session in &mut self.sessions {
            let Some(window) = session
                .windows
                .iter_mut()
                .find_map(|window| window.remove_pane(pane).then_some(window))
            else {
                continue;
            };
            if window.panes.is_empty() {
                let id = window.id;
                removed.window = Some(id);
                let index = session.window_number(id).unwrap() - 1;
                session.windows.retain(|window| !window.panes.is_empty());
                if removed.window == Some(session.last_window)
                    && let Some(next) = session
                        .windows
                        .get(index)
                        .or_else(|| session.windows.last())
                {
                    session.last_window = next.id;
                }
            }
            if session.windows.is_empty() {
                removed.session = Some(session.id);
            }
            break;
        }
        self.sessions.retain(|session| !session.windows.is_empty());
        removed
    }
}

impl Session {
    pub fn window_number(&self, id: WindowId) -> Option<usize> {
        self.windows
            .iter()
            .position(|window| window.id == id)
            .map(|index| index + 1)
    }
}

impl Window {
    pub fn new(id: WindowId, pane: PaneId, size: GridSize) -> Window {
        Window {
            id,
            panes: vec![pane],
            active_pane: pane,
            name: None,
            layout: Layout::Leaf(pane),
            size,
            zoomed: false,
        }
    }

    pub fn area(&self) -> CellRect {
        CellRect {
            origin: CellPoint::default(),
            size: self.size,
        }
    }

    pub fn geometry(&self, band: u16) -> WindowLayout {
        let area = self.area();
        let geometry = self.layout.geometry(area, band);
        let (panes, separators) = if self.zoomed {
            (vec![leaf(self.active_pane, area, band)], vec![])
        } else {
            (geometry.panes, geometry.separators)
        };
        WindowLayout {
            window: self.id,
            area,
            panes,
            separators,
            active: self.active_pane,
            zoomed: self.zoomed,
        }
    }

    fn remove_pane(&mut self, pane: PaneId) -> bool {
        let Some(index) = self.panes.iter().position(|candidate| *candidate == pane) else {
            return false;
        };
        self.panes.remove(index);
        if let Some(layout) = self.layout.clone().remove(pane) {
            self.layout = layout;
        }
        if self.active_pane == pane
            && let Some(next) = self.panes.get(index).or(self.panes.last())
        {
            self.active_pane = *next;
        }
        true
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
