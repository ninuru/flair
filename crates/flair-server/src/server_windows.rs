use super::*;
use flair_core::{
    command::{PaneRef, WindowRef},
    geom::Axis,
    theme::Theme,
    tree::Window,
};
use flair_term::{Terminal, pty::SpawnSpec};
use std::path::PathBuf;

impl Server {
    pub(super) fn spawn_pane(
        &mut self,
        id: PaneId,
        cwd: PathBuf,
        size: GridSize,
        palette: &Theme,
        env: &[(String, String)],
        no_history: bool,
    ) -> SpawnSpec {
        let size = GridSize {
            cols: size.cols.max(1),
            rows: size.rows.saturating_sub(1).max(1),
        };
        let mut terminal = Terminal::new(size, self.config.scrollback);
        terminal.set_palette(palette);
        self.panes.insert(
            id,
            Pane {
                terminal,
                size,
                cwd: cwd.clone(),
                dirty: true,
                cached: None,
                exited: false,
                wheel_copy: false,
            },
        );
        let mut pane_env: BTreeMap<_, _> = self.config.env.iter().cloned().collect();
        pane_env.extend(env.iter().cloned());
        pane_env.insert("TERM_PROGRAM".into(), "flair".into());
        pane_env.insert("FLAIR_PANE".into(), format!("%{}", id.0));
        if no_history {
            pane_env.remove("HISTFILE");
        }
        SpawnSpec {
            program: self.config.program.clone(),
            args: self.config.args.clone(),
            cwd,
            env: pane_env.into_iter().collect(),
            unset_env: ["FLAIR_SOCKET", "FLAIR_SOCKET_NAME"]
                .into_iter()
                .chain(no_history.then_some("HISTFILE"))
                .map(String::from)
                .collect(),
            size,
        }
    }

    pub(super) fn new_window(&mut self, now: Millis, id: ClientId, out: &mut Vec<Effect>) {
        let client = &self.clients[&id];
        let session = self.tree.session(client.view.session).unwrap();
        let (root, no_history, session_id) =
            (session.root.clone(), session.name == "H", session.id);
        let (size, palette, env) = (client.size, client.palette.clone(), client.env.clone());
        let number = self.next_id;
        self.next_id += 1;
        let (window, pane) = (WindowId(number), PaneId(number));
        let spec = self.spawn_pane(pane, root, size, &palette, &env, no_history);
        self.tree
            .sessions
            .iter_mut()
            .find(|session| session.id == session_id)
            .unwrap()
            .windows
            .push(Window::new(window, pane, size));
        out.push(Effect::Spawn(pane, spec));
        self.select_window(now, id, window, out);
    }

    pub(super) fn select_window(
        &mut self,
        now: Millis,
        id: ClientId,
        window: WindowId,
        out: &mut Vec<Effect>,
    ) {
        let client = &self.clients[&id];
        if !self
            .tree
            .session(client.view.session)
            .unwrap()
            .windows
            .iter()
            .any(|candidate| candidate.id == window)
        {
            return;
        }
        self.cancel_mouse(id, out);
        let client = self.clients.get_mut(&id).unwrap();
        let old = client.view.window;
        client.view.select_window(window);
        client.full = true;
        client.selecting = false;
        client.last_click = None;
        self.resize_window(old, out);
        self.touch(now, id, out);
        self.send_title(id, out);
    }

    pub(super) fn window_target(&self, id: ClientId, target: WindowRef) -> Option<WindowId> {
        let session = self.tree.session(self.clients[&id].view.session)?;
        match target {
            WindowRef::Index(index) => session
                .windows
                .get(usize::from(index).checked_sub(1)?)
                .map(|window| window.id),
            WindowRef::Id(id) => session
                .windows
                .iter()
                .find(|window| window.id == id)
                .map(|window| window.id),
        }
    }

    pub(super) fn cycle_window(
        &mut self,
        now: Millis,
        id: ClientId,
        forward: bool,
        out: &mut Vec<Effect>,
    ) {
        let client = &self.clients[&id];
        let session = self.tree.session(client.view.session).unwrap();
        let index = session.window_number(client.view.window).unwrap() - 1;
        let count = session.windows.len();
        let index = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        self.select_window(now, id, session.windows[index].id, out);
    }

    pub(super) fn split(&mut self, now: Millis, id: ClientId, axis: Axis, out: &mut Vec<Effect>) {
        self.touch(now, id, out);
        let client = &self.clients[&id];
        let window_id = client.view.window;
        let parent = self.client_pane(id).unwrap();
        let number = self.next_id;
        let new = PaneId(number);
        let window = self.tree.window_mut(window_id).unwrap();
        let area = window.area();
        if !window.layout.split(parent, new, axis, area) {
            out.push(Effect::Send(
                id,
                ServerMsg::Error("pane is too small to split".into()),
            ));
            return;
        }
        self.next_id += 1;
        window.panes.push(new);
        window.active_pane = new;
        window.zoomed = false;
        let pane = &self.panes[&parent];
        let reported = pane.terminal.cwd().map(PathBuf::from);
        let cwd = reported.clone().unwrap_or_else(|| pane.cwd.clone());
        let client = &self.clients[&id];
        let (size, palette, env) = (client.size, client.palette.clone(), client.env.clone());
        let no_history = self.tree.session(client.view.session).unwrap().name == "H";
        let mut spec = self.spawn_pane(new, cwd, size, &palette, &env, no_history);
        let placement = self
            .tree
            .window(window_id)
            .unwrap()
            .geometry()
            .panes
            .into_iter()
            .find(|placement| placement.pane == new)
            .unwrap();
        spec.size = GridSize {
            cols: placement.rect.size.cols.max(1),
            rows: placement.rect.size.rows.max(1),
        };
        let pane = self.panes.get_mut(&new).unwrap();
        pane.size = spec.size;
        pane.terminal.resize(spec.size);
        out.push(if reported.is_some() {
            Effect::Spawn(new, spec)
        } else {
            Effect::SpawnSplit(new, parent, spec)
        });
        self.invalidate_window(window_id, out);
    }

    pub(super) fn select_pane(&mut self, id: ClientId, target: PaneRef, out: &mut Vec<Effect>) {
        let window_id = self.clients[&id].view.window;
        let window = self.tree.window(window_id).unwrap();
        let pane = match target {
            PaneRef::Id(pane) => window.panes.contains(&pane).then_some(pane),
            PaneRef::Direction(direction) => {
                window
                    .layout
                    .neighbor(window.active_pane, direction, window.area())
            }
        };
        if let Some(pane) = pane {
            self.tree.window_mut(window_id).unwrap().active_pane = pane;
            self.invalidate_window(window_id, out);
        }
    }

    pub(super) fn invalidate_window(&mut self, window: WindowId, out: &mut Vec<Effect>) {
        let clients: Vec<_> = self
            .clients
            .iter()
            .filter(|(_, client)| client.view.window == window)
            .map(|(id, _)| *id)
            .collect();
        for id in clients {
            self.cancel_mouse(id, out);
            let client = self.clients.get_mut(&id).unwrap();
            client.full = true;
            client.last_click = None;
        }
        self.resize_window(window, out);
        let clients: Vec<_> = self
            .clients
            .iter()
            .filter(|(_, client)| client.view.window == window)
            .map(|(id, _)| *id)
            .collect();
        for id in clients {
            self.send_title(id, out);
        }
    }
}
