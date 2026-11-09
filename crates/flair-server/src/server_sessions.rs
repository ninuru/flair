use super::*;
use flair_core::{
    config::Config,
    proto::SessionInfo,
    theme::Theme,
    tree::{Session, Window},
};

impl Server {
    pub(super) fn resolve_target(
        &mut self,
        target: Target,
        size: GridSize,
        palette: &Theme,
        env: &[(String, String)],
        out: &mut Vec<Effect>,
    ) -> Result<SessionId, String> {
        let (name, root, create_only) = match target {
            Target::Main => ("main".into(), self.config.cwd.clone(), false),
            Target::Name(name) => {
                return self
                    .tree
                    .sessions
                    .iter()
                    .find(|session| session.name == name)
                    .map(|session| session.id)
                    .ok_or_else(|| format!("no session named {name}"));
            }
            Target::New => (self.numeric_name(), self.config.cwd.clone(), true),
            Target::Create { name, root } => {
                (name.unwrap_or_else(|| self.numeric_name()), root, true)
            }
            Target::Path(root) => {
                if let Some(session) = self
                    .tree
                    .sessions
                    .iter()
                    .find(|session| session.root == root)
                {
                    return Ok(session.id);
                }
                let basename = root
                    .file_name()
                    .ok_or("session root must name a directory")?
                    .to_string_lossy()
                    .replace('.', "_");
                let mut name = basename.clone();
                let mut suffix = 2;
                while self
                    .tree
                    .sessions
                    .iter()
                    .any(|session| session.name == name)
                {
                    name = format!("{basename}_{suffix}");
                    suffix += 1;
                }
                (name, root, true)
            }
        };
        if name.is_empty()
            || name.len() > 255
            || name.chars().any(char::is_control)
            || name.contains('/')
        {
            return Err("session name must be nonempty and contain no controls or slashes".into());
        }
        if !root.is_absolute() {
            return Err("session root must be an absolute path".into());
        }
        if let Some(session) = self
            .tree
            .sessions
            .iter()
            .find(|session| session.name == name)
        {
            return if create_only {
                Err(format!("session {name} already exists"))
            } else {
                Ok(session.id)
            };
        }
        let number = self.next_id;
        self.next_id += 1;
        let session = SessionId(number);
        let window = WindowId(number);
        let pane_id = PaneId(number);
        let no_history = name == "H";
        let spec = self.spawn_pane(pane_id, root.clone(), size, palette, env, no_history);
        self.tree.sessions.push(Session {
            id: session,
            name,
            root,
            last_window: window,
            windows: vec![Window::new(window, pane_id, size)],
        });
        out.push(Effect::Spawn(pane_id, spec));
        Ok(session)
    }

    fn numeric_name(&self) -> String {
        let mut number = 1;
        while self
            .tree
            .sessions
            .iter()
            .any(|session| session.name == number.to_string())
        {
            number += 1;
        }
        number.to_string()
    }

    pub(super) fn session_infos(&self) -> Vec<SessionInfo> {
        self.tree
            .sessions
            .iter()
            .map(|session| SessionInfo {
                name: session.name.clone(),
                root: session.root.clone(),
                windows: session.windows.len(),
            })
            .collect()
    }

    pub(super) fn switch_to(
        &mut self,
        now: Millis,
        id: ClientId,
        session: SessionId,
        out: &mut Vec<Effect>,
    ) {
        let window = self.tree.session(session).unwrap().last_window;
        self.cancel_mouse(id, out);
        let client = self.clients.get_mut(&id).unwrap();
        let previous_window = client.view.window;
        client.view.switch_session(session, window);
        client.full = true;
        client.prefix.reset();
        client.selecting = false;
        client.last_click = None;
        self.resize_window(previous_window, out);
        self.touch(now, id, out);
        self.send_title(id, out);
    }

    pub(super) fn control(
        &mut self,
        now: Millis,
        id: ClientId,
        control: Control,
        out: &mut Vec<Effect>,
    ) {
        match control {
            Control::List => {
                out.push(Effect::Send(id, ServerMsg::Sessions(self.session_infos())));
                out.push(Effect::Send(id, ServerMsg::Done));
            }
            Control::KillServer => {
                for client in self.clients.keys() {
                    out.push(Effect::Send(
                        *client,
                        ServerMsg::Exit(ExitReason::ServerExited),
                    ));
                }
                for pane in self.panes.keys() {
                    out.push(Effect::KillPty(*pane));
                }
                out.push(Effect::Send(id, ServerMsg::Done));
                out.push(Effect::Shutdown);
                self.stopping = true;
            }
            Control::Create { target, env } => {
                let palette = Config::default().theme();
                match self.resolve_target(
                    target,
                    GridSize { cols: 80, rows: 24 },
                    &palette,
                    &env,
                    out,
                ) {
                    Ok(session) => {
                        let info = self
                            .session_infos()
                            .into_iter()
                            .find(|info| info.name == self.tree.session(session).unwrap().name)
                            .unwrap();
                        out.push(Effect::Send(id, ServerMsg::Sessions(vec![info])));
                        out.push(Effect::Send(id, ServerMsg::Done));
                    }
                    Err(error) => out.push(Effect::Send(id, ServerMsg::Error(error))),
                }
            }
            Control::Switch { target, pane, env } => {
                let chosen = self
                    .clients
                    .iter()
                    .filter(|(client, state)| {
                        state.kind == ClientKind::Gui
                            && pane.is_none_or(|pane| self.client_pane(**client) == Some(pane))
                    })
                    .max_by_key(|(_, state)| (state.focused, state.activity))
                    .map(|(client, _)| *client);
                let (size, palette) = chosen
                    .map(|client| {
                        let state = &self.clients[&client];
                        (state.size, state.palette.clone())
                    })
                    .unwrap_or_else(|| {
                        (GridSize { cols: 80, rows: 24 }, Config::default().theme())
                    });
                match self.resolve_target(target, size, &palette, &env, out) {
                    Ok(session) => {
                        if let Some(client) = chosen {
                            self.switch_to(now, client, session, out);
                        } else {
                            let info = self
                                .session_infos()
                                .into_iter()
                                .find(|info| info.name == self.tree.session(session).unwrap().name)
                                .unwrap();
                            out.push(Effect::Send(id, ServerMsg::Sessions(vec![info])));
                        }
                        out.push(Effect::Send(id, ServerMsg::Done));
                    }
                    Err(error) => out.push(Effect::Send(id, ServerMsg::Error(error))),
                }
            }
        }
    }
}
