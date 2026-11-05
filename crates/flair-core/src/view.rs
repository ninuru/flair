use crate::{
    geom::GridSize,
    ids::{SessionId, WindowId},
    time::Millis,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientView {
    pub session: SessionId,
    pub window: WindowId,
    pub last_window: Option<WindowId>,
    pub last_session: Option<SessionId>,
    pub last_active: Millis,
}

impl ClientView {
    pub fn new(session: SessionId, window: WindowId, now: Millis) -> ClientView {
        ClientView {
            session,
            window,
            last_window: None,
            last_session: None,
            last_active: now,
        }
    }

    pub fn select_window(&mut self, window: WindowId) {
        if window != self.window {
            self.last_window = Some(self.window);
            self.window = window;
        }
    }

    pub fn switch_session(&mut self, session: SessionId, window: WindowId) {
        if session == self.session {
            self.select_window(window);
            return;
        }
        self.last_session = Some(self.session);
        self.session = session;
        self.window = window;
        self.last_window = None;
    }

    pub fn touch(&mut self, now: Millis) {
        self.last_active = now;
    }
}

pub fn window_size<'a>(
    clients: impl Iterator<Item = (&'a ClientView, GridSize)>,
    window: WindowId,
) -> Option<GridSize> {
    clients
        .filter(|(view, _)| view.window == window)
        .max_by_key(|(view, _)| view.last_active)
        .map(|(_, size)| size)
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
