use std::path::PathBuf;

use crate::ids::{PaneId, SessionId, WindowId};

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
                windows: vec![Window {
                    id: window,
                    panes: vec![pane],
                    active_pane: pane,
                }],
            }],
        }
    }

    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn active_pane(&self, window: WindowId) -> Option<PaneId> {
        self.sessions
            .iter()
            .flat_map(|session| &session.windows)
            .find(|candidate| candidate.id == window)
            .map(|found| found.active_pane)
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
                removed.window = Some(window.id);
                session.windows.retain(|window| !window.panes.is_empty());
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

impl Window {
    fn remove_pane(&mut self, pane: PaneId) -> bool {
        let Some(index) = self.panes.iter().position(|candidate| *candidate == pane) else {
            return false;
        };
        self.panes.remove(index);
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
