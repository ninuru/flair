use super::*;
use flair_core::{
    command::{Command, WindowRef},
    config::Config,
    proto::ClientKind,
    status::StatusWindow,
};

fn hello() -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        kind: ClientKind::Gui,
        size: GridSize { cols: 20, rows: 4 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target: Target::Main,
    }
}

fn command(server: &mut Server, at: u64, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(at),
        Event::Client(ClientId(1), ClientMsg::Command(command)),
        &mut out,
    );
    out
}

fn two_sessions() -> Server {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    command(&mut server, 1, Command::NewWindow);
    command(
        &mut server,
        2,
        Command::Switch(Target::Path("/tmp/project".into())),
    );
    command(&mut server, 3, Command::NewWindow);
    command(&mut server, 4, Command::NewWindow);
    command(&mut server, 5, Command::SelectWindow(WindowRef::Index(2)));
    server
}

fn chooser(out: &[Effect]) -> Option<(Vec<SessionNode>, SessionId)> {
    out.iter().find_map(|effect| match effect {
        Effect::Send(
            ClientId(1),
            ServerMsg::ChooseSession {
                sessions, current, ..
            },
        ) => Some((sessions.clone(), *current)),
        Effect::Send(_, _)
        | Effect::Spawn(_, _)
        | Effect::SpawnSplit(_, _, _)
        | Effect::WritePty(_, _)
        | Effect::ResizePty(_, _)
        | Effect::KillPty(_)
        | Effect::SetTimer(_, _)
        | Effect::ReadConfig(_)
        | Effect::ReadGit { .. }
        | Effect::ReadContexts
        | Effect::ReadProcess(_)
        | Effect::PaneProcesses { .. }
        | Effect::Shutdown => None,
    })
}

fn flags(windows: &[StatusWindow]) -> Vec<(u16, bool, bool)> {
    windows
        .iter()
        .map(|window| (window.index, window.active, window.last))
        .collect()
}

#[test]
fn choose_session_sends_every_session_with_its_windows_and_the_current_id() {
    let mut server = two_sessions();
    let out = command(&mut server, 6, Command::ChooseSession);
    let (sessions, current) = chooser(&out).expect("the chooser is sent");
    let project = server.clients[&ClientId(1)].view.session;
    assert_eq!(current, project);
    let names: Vec<_> = sessions
        .iter()
        .map(|node| (node.name.as_str(), node.root.clone()))
        .collect();
    assert_eq!(
        names,
        [
            ("main", server.tree.sessions[0].root.clone()),
            ("project", "/tmp/project".into())
        ]
    );
    assert_eq!(sessions[1].id, project);
    assert_eq!(
        flags(&sessions[0].windows),
        [(1, false, false), (2, true, false)]
    );
    assert_eq!(
        flags(&sessions[1].windows),
        [(1, false, false), (2, true, false), (3, false, true)]
    );
    let ids: Vec<_> = sessions[1].windows.iter().map(|window| window.id).collect();
    let tree: Vec<_> = server.tree.sessions[1]
        .windows
        .iter()
        .map(|window| window.id)
        .collect();
    assert_eq!(ids, tree);
}

#[test]
fn select_window_by_id_in_another_session_switches_session_first() {
    let mut server = two_sessions();
    let main = server.tree.sessions[0].id;
    let target = server.tree.sessions[0].windows[0].id;
    let out = command(&mut server, 6, Command::SelectWindow(WindowRef::Id(target)));
    let view = &server.clients[&ClientId(1)].view;
    assert_eq!((view.session, view.window), (main, target));
    assert_eq!(view.last_session, Some(server.tree.sessions[1].id));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Title(_))))
    );
}

#[test]
fn select_window_by_id_in_another_session_goes_straight_to_that_window() {
    let mut server = two_sessions();
    let mut out = Vec::new();
    server.handle(
        Millis(6),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 40, rows: 10 }),
        ),
        &mut out,
    );
    let main = &server.tree.sessions[0];
    let (main, target, last) = (main.id, main.windows[0].id, main.last_window);
    assert_ne!(target, last);
    let skipped = server.tree.window(last).unwrap().panes.clone();
    let chosen = server.tree.window(target).unwrap().panes.clone();
    let out = command(&mut server, 7, Command::SelectWindow(WindowRef::Id(target)));
    let resized: Vec<PaneId> = out
        .iter()
        .filter_map(|effect| match effect {
            Effect::ResizePty(pane, _) => Some(*pane),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::KillPty(_)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect();
    assert!(resized.iter().all(|pane| !skipped.contains(pane)));
    assert!(chosen.iter().all(|pane| resized.contains(pane)));
    let titles = out
        .iter()
        .filter(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Title(_))))
        .count();
    let statuses = out
        .iter()
        .filter(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Status(_))))
        .count();
    assert_eq!((titles, statuses), (1, 1));
    let view = &server.clients[&ClientId(1)].view;
    assert_eq!((view.session, view.window), (main, target));
    assert_eq!(server.tree.session(main).unwrap().last_window, target);
}

#[test]
fn select_window_by_id_in_the_current_session_only_selects_it() {
    let mut server = two_sessions();
    let project = server.tree.sessions[1].id;
    let target = server.tree.sessions[1].windows[0].id;
    command(&mut server, 6, Command::SelectWindow(WindowRef::Id(target)));
    let view = &server.clients[&ClientId(1)].view;
    assert_eq!((view.session, view.window), (project, target));
    assert_eq!(view.last_session, Some(server.tree.sessions[0].id));
}

#[test]
fn select_window_by_unknown_id_changes_nothing() {
    let mut server = two_sessions();
    let before = server.clients[&ClientId(1)].view;
    command(
        &mut server,
        6,
        Command::SelectWindow(WindowRef::Id(WindowId(999))),
    );
    let after = server.clients[&ClientId(1)].view;
    assert_eq!(
        (
            after.session,
            after.window,
            after.last_window,
            after.last_session
        ),
        (
            before.session,
            before.window,
            before.last_window,
            before.last_session
        )
    );
}
