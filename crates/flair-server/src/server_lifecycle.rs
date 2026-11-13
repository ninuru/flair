use super::*;

impl Server {
    pub(super) fn reap_sessions(&mut self, now: Millis, out: &mut Vec<Effect>) {
        let finished: Vec<_> = self
            .panes
            .iter()
            .filter(|(pane_id, pane)| {
                pane.exited
                    && !pane.dirty
                    && self
                        .clients
                        .values()
                        .filter(|client| {
                            self.tree.window(client.view.window).is_some_and(|window| {
                                window
                                    .geometry(0)
                                    .panes
                                    .iter()
                                    .any(|placement| placement.pane == **pane_id)
                            })
                        })
                        .all(|client| {
                            pane.cached.is_none()
                                || (client.acknowledged.get(pane_id) == pane.cached.as_ref()
                                    && (client.awaiting_ack.is_none()
                                        || client.previous.get(pane_id) == pane.cached.as_ref()))
                        })
            })
            .map(|(id, _)| *id)
            .collect();
        for pane in finished {
            self.destroy_pane(now, pane, out);
        }
        for client in self.clients.values_mut() {
            client
                .finished
                .retain(|session| self.tree.session(*session).is_some());
        }
    }

    pub(super) fn destroy_pane(&mut self, now: Millis, pane: PaneId, out: &mut Vec<Effect>) {
        let window = self.tree.sessions.iter().find_map(|session| {
            session
                .windows
                .iter()
                .enumerate()
                .find_map(|(index, window)| {
                    window.panes.contains(&pane).then_some((window.id, index))
                })
        });
        let window_position = window.map(|(_, index)| index);
        let remembered = self
            .tree
            .sessions
            .iter()
            .find(|session| window.is_some_and(|(id, _)| session.last_window == id))
            .map(|session| session.id);
        self.set_started(pane, None);
        if self.panes.remove(&pane).is_none() {
            return;
        }
        out.push(Effect::KillPty(pane));
        let removed = self.tree.remove_pane(pane);
        if let Some(window) = removed.window {
            let window_index = window_position.unwrap();
            let affected: Vec<_> = self
                .clients
                .iter()
                .filter(|(_, client)| client.view.window == window)
                .map(|(id, _)| *id)
                .collect();
            for id in affected {
                let client = self.clients.get_mut(&id).unwrap();
                let Some(session) = self.tree.session(client.view.session) else {
                    continue;
                };
                let next = client
                    .view
                    .last_window
                    .filter(|last| session.windows.iter().any(|window| window.id == *last))
                    .unwrap_or_else(|| {
                        session
                            .windows
                            .get(window_index)
                            .or_else(|| session.windows.last())
                            .unwrap()
                            .id
                    });
                client.view.select_window(next);
                client.view.last_window = None;
                client.full = true;
                client.selecting = false;
                client.last_click = None;
                self.resize_window(next, out);
                self.send_title(id, out);
            }
            for client in self.clients.values_mut() {
                if client.view.last_window == Some(window) {
                    client.view.last_window = None;
                }
            }
        }
        if removed.window.is_some()
            && let Some(session_id) = remembered
            && let Some(window) = self
                .clients
                .values()
                .filter(|client| client.view.session == session_id)
                .max_by_key(|client| client.activity)
                .map(|client| client.view.window)
            && let Some(session) = self
                .tree
                .sessions
                .iter_mut()
                .find(|session| session.id == session_id)
        {
            session.last_window = window;
        }
        if let Some(session) = removed.session {
            let affected: Vec<_> = self
                .clients
                .iter()
                .filter(|(_, client)| client.view.session == session)
                .map(|(id, _)| *id)
                .collect();
            for id in affected {
                let last = self.clients[&id]
                    .view
                    .last_session
                    .filter(|last| self.tree.session(*last).is_some());
                if let Some(last) = last {
                    self.switch_to(now, id, last, None, out);
                } else {
                    self.clients.remove(&id);
                    out.push(Effect::Send(id, ServerMsg::Exit(ExitReason::SessionEnded)));
                }
            }
        }
        if let Some((window, _)) = window
            && self.tree.window(window).is_some()
        {
            self.invalidate_window(window, out);
        }
        if self.next_id > 1 && self.tree.sessions.is_empty() {
            self.stopping = true;
            out.push(Effect::Shutdown);
        }
    }
}
