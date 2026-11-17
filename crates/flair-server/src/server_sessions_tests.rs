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
        | Effect::Notify { .. }
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
            | Effect::Notify { .. }
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

fn attach(server: &mut Server, at: u64, client: u32, target: Target) -> Vec<Effect> {
    let mut out = Vec::new();
    let ClientMsg::Hello {
        protocol,
        kind,
        size,
        palette,
        env,
        ..
    } = hello()
    else {
        unreachable!()
    };
    server.handle(
        Millis(at),
        Event::Client(
            ClientId(client),
            ClientMsg::Hello {
                protocol,
                kind,
                size,
                palette,
                env,
                target,
            },
        ),
        &mut out,
    );
    out
}

fn session_named(server: &Server, name: &str) -> Option<SessionId> {
    server
        .tree
        .sessions
        .iter()
        .find(|session| session.name == name)
        .map(|session| session.id)
}

fn session_panes(server: &Server, session: SessionId) -> Vec<PaneId> {
    server
        .tree
        .session(session)
        .unwrap()
        .windows
        .iter()
        .flat_map(|window| window.panes.clone())
        .collect()
}

fn killed(out: &[Effect]) -> Vec<PaneId> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::KillPty(pane) => Some(*pane),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Notify { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn temp_beside_main() -> (Server, SessionId, Vec<PaneId>) {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 2, Target::Main);
    attach(&mut server, 1, 1, Target::Temp);
    command(&mut server, 2, Command::NewWindow);
    let temp = server.clients[&ClientId(1)].view.session;
    let panes = session_panes(&server, temp);
    assert_eq!(panes.len(), 2);
    (server, temp, panes)
}

#[test]
fn temp_hello_creates_a_new_temporary_numbered_session_beside_main() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 2, Target::Main);
    let out = attach(&mut server, 1, 1, Target::Temp);
    let temp = server.clients[&ClientId(1)].view.session;
    let session = server.tree.session(temp).unwrap();
    assert_eq!(session.name, "1");
    assert!(session.temporary);
    assert_eq!(session.root, server.config.cwd);
    assert!(!server.tree.sessions[0].temporary);
    assert_eq!(server.tree.sessions.len(), 2);
    let pane = session.windows[0].panes[0];
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Spawn(spawned, _) if *spawned == pane))
    );
}

#[test]
fn session_nodes_and_infos_carry_the_temporary_flag() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 2, Target::Main);
    attach(&mut server, 1, 1, Target::Temp);
    let nodes: Vec<_> = server
        .session_nodes(ClientId(1))
        .into_iter()
        .map(|node| (node.name, node.temporary))
        .collect();
    assert_eq!(nodes, [("main".to_owned(), false), ("1".to_owned(), true)]);
    let infos: Vec<_> = server
        .session_infos()
        .into_iter()
        .map(|info| (info.name, info.temporary))
        .collect();
    assert_eq!(infos, [("main".to_owned(), false), ("1".to_owned(), true)]);
}

#[test]
fn the_last_client_leaving_a_temp_session_kills_its_panes_and_removes_it() {
    let leaves: [fn(&mut Server) -> Vec<Effect>; 3] = [
        |server| {
            let mut out = Vec::new();
            server.handle(
                Millis(10),
                Event::Client(ClientId(1), ClientMsg::Detach),
                &mut out,
            );
            out
        },
        |server| {
            let mut out = Vec::new();
            server.handle(Millis(10), Event::ClientGone(ClientId(1)), &mut out);
            out
        },
        |server| command(server, 10, Command::Detach),
    ];
    for leave in leaves {
        let (mut server, temp, panes) = temp_beside_main();
        let out = leave(&mut server);
        let mut gone = killed(&out);
        gone.sort();
        assert_eq!(gone, panes);
        assert!(server.tree.session(temp).is_none());
        assert!(session_named(&server, "main").is_some());
        assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
    }
}

#[test]
fn the_only_temp_session_ending_shuts_the_daemon_down() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 1, Target::Temp);
    let out = command(&mut server, 1, Command::Detach);
    assert_eq!(killed(&out).len(), 1);
    assert!(server.tree.sessions.is_empty());
    assert!(out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn regular_sessions_survive_detach() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 1, Target::Main);
    let out = command(&mut server, 1, Command::Detach);
    assert!(killed(&out).is_empty());
    assert!(session_named(&server, "main").is_some());
    attach(
        &mut server,
        2,
        1,
        Target::Create {
            name: Some("work".into()),
            root: "/tmp".into(),
        },
    );
    let out = command(&mut server, 3, Command::Detach);
    assert!(killed(&out).is_empty());
    assert!(session_named(&server, "work").is_some());
}

#[test]
fn switching_away_keeps_the_temp_session_as_last_session_until_a_third_switch() {
    let (mut server, temp, panes) = temp_beside_main();
    let out = command(
        &mut server,
        10,
        Command::Switch(Target::Name("main".into())),
    );
    assert!(killed(&out).is_empty());
    assert_eq!(server.clients[&ClientId(1)].view.last_session, Some(temp));
    command(&mut server, 11, Command::LastSession);
    assert_eq!(server.clients[&ClientId(1)].view.session, temp);
    command(
        &mut server,
        12,
        Command::Switch(Target::Name("main".into())),
    );
    let out = command(
        &mut server,
        13,
        Command::Switch(Target::Path("/tmp/third".into())),
    );
    let mut gone = killed(&out);
    gone.sort();
    assert_eq!(gone, panes);
    assert!(server.tree.session(temp).is_none());
    assert_eq!(server.clients.len(), 2);
}

#[test]
fn closing_the_last_window_of_a_temp_session_reached_from_elsewhere_ends_it() {
    let (mut server, temp, _) = temp_beside_main();
    command(
        &mut server,
        10,
        Command::Switch(Target::Name("main".into())),
    );
    command(&mut server, 11, Command::LastSession);
    let windows: Vec<_> = server
        .tree
        .session(temp)
        .unwrap()
        .windows
        .iter()
        .map(|window| window.id)
        .collect();
    for window in windows {
        command(&mut server, 12, Command::KillWindow(window));
    }
    assert!(server.tree.session(temp).is_none());
    let main = session_named(&server, "main").unwrap();
    assert_eq!(server.clients[&ClientId(1)].view.session, main);
}

#[test]
fn another_client_viewing_a_temp_session_keeps_it_alive() {
    let (mut server, temp, panes) = temp_beside_main();
    attach(&mut server, 10, 3, Target::Name("1".into()));
    let out = command(&mut server, 11, Command::Detach);
    assert!(killed(&out).is_empty());
    assert!(server.tree.session(temp).is_some());
    let mut out = Vec::new();
    server.handle(Millis(12), Event::ClientGone(ClientId(3)), &mut out);
    let mut gone = killed(&out);
    gone.sort();
    assert_eq!(gone, panes);
    assert!(server.tree.session(temp).is_none());
}

#[test]
fn path_targets_do_not_reuse_a_temp_session_with_the_same_root() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    attach(&mut server, 0, 1, Target::Temp);
    let temp = server.clients[&ClientId(1)].view.session;
    let root = server.config.cwd.clone();
    command(&mut server, 1, Command::Switch(Target::Path(root.clone())));
    let session = server.clients[&ClientId(1)].view.session;
    assert_ne!(session, temp);
    let session = server.tree.session(session).unwrap();
    assert_eq!(session.root, root);
    assert!(!session.temporary);
    assert!(server.tree.session(temp).is_some());
}
