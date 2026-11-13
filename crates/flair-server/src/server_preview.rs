use super::*;
use crate::client::PreviewWatch;
use flair_core::{
    preview::ink,
    proto::{PanePlacement, PanePreview, SessionPreview, WindowLayout, WindowPreview},
};

const WINDOWS: usize = 9;
const INTERVAL: Millis = Millis(250);

impl Server {
    pub(super) fn watch(
        &mut self,
        now: Millis,
        id: ClientId,
        session: SessionId,
    ) -> Option<SessionPreview> {
        let preview = self.session_preview(session);
        self.clients.get_mut(&id).unwrap().preview = preview.as_ref().map(|_| PreviewWatch {
            session,
            sent: now,
            stale: false,
            armed: false,
        });
        preview
    }

    pub(super) fn watch_preview(
        &mut self,
        now: Millis,
        id: ClientId,
        session: Option<SessionId>,
        out: &mut Vec<Effect>,
    ) {
        match session {
            Some(session) => match self.clients[&id].preview {
                Some(watch)
                    if now < watch.sent + INTERVAL && self.tree.session(session).is_some() =>
                {
                    if !watch.armed {
                        out.push(Effect::SetTimer(Timer::Preview(id), watch.sent + INTERVAL));
                    }
                    self.clients.get_mut(&id).unwrap().preview = Some(PreviewWatch {
                        session,
                        stale: true,
                        armed: true,
                        ..watch
                    });
                }
                Some(_) | None => {
                    if let Some(preview) = self.watch(now, id, session) {
                        out.push(Effect::Send(id, ServerMsg::Preview(preview)));
                    }
                }
            },
            None => self.clients.get_mut(&id).unwrap().preview = None,
        }
    }

    pub(super) fn preview_timer(&mut self, client: ClientId) {
        if let Some(watch) = self
            .clients
            .get_mut(&client)
            .and_then(|client| client.preview.as_mut())
        {
            watch.armed = false;
        }
    }

    pub(super) fn flush_previews(
        &mut self,
        now: Millis,
        changed: &BTreeSet<PaneId>,
        out: &mut Vec<Effect>,
    ) {
        let watching: Vec<(ClientId, PreviewWatch)> = self
            .clients
            .iter()
            .filter_map(|(id, client)| client.preview.map(|watch| (*id, watch)))
            .collect();
        for (id, watch) in watching {
            let touched = self.tree.session(watch.session).map(|session| {
                session
                    .windows
                    .iter()
                    .take(WINDOWS)
                    .flat_map(|window| &window.panes)
                    .any(|pane| changed.contains(pane))
            });
            let next = match touched {
                Some(touched) => Some(self.push_preview(now, id, watch, touched, out)),
                None => {
                    out.push(Effect::Send(
                        id,
                        ServerMsg::Preview(SessionPreview {
                            session: watch.session,
                            windows: Vec::new(),
                        }),
                    ));
                    None
                }
            };
            self.clients.get_mut(&id).unwrap().preview = next;
        }
    }

    fn push_preview(
        &mut self,
        now: Millis,
        id: ClientId,
        watch: PreviewWatch,
        touched: bool,
        out: &mut Vec<Effect>,
    ) -> PreviewWatch {
        let due = watch.sent + INTERVAL;
        match (watch.stale || touched, now >= due, watch.armed) {
            (false, _, _) => watch,
            (true, true, _) => {
                if let Some(preview) = self.session_preview(watch.session) {
                    out.push(Effect::Send(id, ServerMsg::Preview(preview)));
                }
                PreviewWatch {
                    sent: now,
                    stale: false,
                    ..watch
                }
            }
            (true, false, true) => PreviewWatch {
                stale: true,
                ..watch
            },
            (true, false, false) => {
                out.push(Effect::SetTimer(Timer::Preview(id), due));
                PreviewWatch {
                    stale: true,
                    armed: true,
                    ..watch
                }
            }
        }
    }

    fn session_preview(&mut self, session: SessionId) -> Option<SessionPreview> {
        let layouts: Vec<WindowLayout> = self
            .tree
            .session(session)?
            .windows
            .iter()
            .take(WINDOWS)
            .map(|window| window.geometry(0))
            .collect();
        let windows = layouts
            .into_iter()
            .map(|layout| WindowPreview {
                window: layout.window,
                panes: layout
                    .panes
                    .iter()
                    .map(|placement| self.pane_preview(*placement))
                    .collect(),
                layout,
            })
            .collect();
        Some(SessionPreview { session, windows })
    }

    fn pane_preview(&mut self, placement: PanePlacement) -> PanePreview {
        let size = placement.rect.size;
        let rows = match self.panes.get_mut(&placement.pane) {
            Some(pane) => match &pane.cached {
                Some(cached) => ink(&cached.rows, size),
                None => pane
                    .terminal
                    .snapshot(placement.pane, true)
                    .map_or_else(Vec::new, |snapshot| ink(&snapshot.rows, size)),
            },
            None => Vec::new(),
        };
        PanePreview {
            pane: placement.pane,
            rows,
        }
    }
}

#[cfg(test)]
#[path = "server_preview_tests.rs"]
mod tests;
