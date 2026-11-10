use super::*;

impl Server {
    pub(super) fn resize_window(&mut self, window: WindowId, out: &mut Vec<Effect>) {
        let mut geometry_changed = false;
        if let Some(size) = self
            .clients
            .values()
            .filter(|client| client.view.window == window)
            .max_by_key(|client| client.activity)
            .map(|client| client.size)
            && let Some(window) = self.tree.window_mut(window)
        {
            geometry_changed = window.size != size;
            window.size = size;
        }
        if geometry_changed {
            let clients: Vec<_> = self
                .clients
                .iter()
                .filter(|(_, client)| client.view.window == window)
                .map(|(id, _)| *id)
                .collect();
            for id in clients {
                self.cancel_mouse(id, out);
            }
        }
        let Some(geometry) = self
            .tree
            .window(window)
            .map(|window| window.geometry(self.band()))
        else {
            return;
        };
        let mut resized = false;
        for placement in geometry.panes {
            let size = GridSize {
                cols: placement.rect.size.cols.max(1),
                rows: placement.rect.size.rows.max(1),
            };
            let Some(pane) = self.panes.get_mut(&placement.pane) else {
                continue;
            };
            if pane.size != size {
                pane.size = size;
                pane.terminal.resize(size);
                pane.dirty = true;
                resized = true;
                out.push(Effect::ResizePty(placement.pane, size));
            }
        }
        if resized {
            for client in self
                .clients
                .values_mut()
                .filter(|client| client.view.window == window)
            {
                client.full = true;
            }
        }
    }

    pub(super) fn flush_frames(&mut self, out: &mut Vec<Effect>) {
        for (id, pane) in &mut self.panes {
            if pane.dirty {
                pane.cached = pane.terminal.snapshot(*id, true);
                pane.dirty = false;
            }
        }
        let band = self.band();
        for (id, client) in &mut self.clients {
            if client.awaiting_ack.is_some() {
                continue;
            }
            let Some(layout) = self
                .tree
                .window(client.view.window)
                .map(|window| window.geometry(band))
            else {
                continue;
            };
            if client.layout.as_ref() != Some(&layout) {
                out.push(Effect::Send(*id, ServerMsg::Layout(layout.clone())));
                client.layout = Some(layout.clone());
            }
            let current: BTreeMap<_, _> = layout
                .panes
                .iter()
                .filter_map(|placement| {
                    self.panes
                        .get(&placement.pane)?
                        .cached
                        .as_ref()
                        .map(|pane| (placement.pane, pane.clone()))
                })
                .collect();
            if !client.full && client.previous == current {
                continue;
            }
            let mut updates = Vec::new();
            for (pane_id, pane) in &current {
                if !client.full && client.previous.get(pane_id) == Some(pane) {
                    continue;
                }
                let mut update = pane.clone();
                if !client.full
                    && let Some(previous) = client.previous.get(pane_id)
                    && previous.size == pane.size
                    && previous.flags == pane.flags
                {
                    update.full = false;
                    update
                        .rows
                        .retain(|(row, cells)| previous.rows[usize::from(*row)].1 != *cells);
                }
                updates.push(update);
            }
            client.previous = current;
            client.full = false;
            if updates.is_empty() {
                continue;
            }
            let seq = client.next_seq;
            client.next_seq += 1;
            client.awaiting_ack = Some(seq);
            out.push(Effect::Send(
                *id,
                ServerMsg::Frame {
                    seq,
                    panes: updates,
                },
            ));
        }
    }
}
