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

    pub fn remove_pane(&mut self, pane: PaneId) -> bool {
        for session in &mut self.sessions {
            for window in &mut session.windows {
                window.remove_pane(pane);
            }
            session.windows.retain(|window| !window.panes.is_empty());
        }
        self.sessions.retain(|session| !session.windows.is_empty());
        self.sessions.is_empty()
    }
}

impl Window {
    fn remove_pane(&mut self, pane: PaneId) {
        let Some(index) = self.panes.iter().position(|candidate| *candidate == pane) else {
            return;
        };
        self.panes.remove(index);
        if self.active_pane == pane
            && let Some(next) = self.panes.get(index).or(self.panes.last())
        {
            self.active_pane = *next;
        }
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
