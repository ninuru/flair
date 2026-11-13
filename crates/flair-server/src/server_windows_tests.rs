use super::*;
use flair_core::geom::Axis;

fn second_window(server: &mut Server) -> (WindowId, PaneId) {
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(99),
            ClientMsg::Control(Control::Create {
                target: Target::Create {
                    name: Some("temporary".into()),
                    root: "/tmp".into(),
                },
                env: vec![],
            }),
        ),
        &mut out,
    );
    let session = server.tree.sessions.pop().unwrap();
    let window = session.windows.into_iter().next().unwrap();
    let result = (window.id, window.active_pane);
    server.tree.sessions[0].windows.push(window);
    result
}

#[test]
fn closing_selected_window_returns_to_surviving_window() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let (window, pane) = second_window(&mut server);
    server
        .clients
        .get_mut(&ClientId(1))
        .unwrap()
        .view
        .select_window(window);
    let mut out = Vec::new();
    server.handle(Millis(2), Event::PtyExited(pane), &mut out);
    let final_frame = ack(&mut server, &out);
    ack(&mut server, &final_frame);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
    assert!(
        server
            .tree
            .active_pane(server.clients[&ClientId(1)].view.window)
            .is_some()
    );
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn clients_in_different_windows_size_only_their_visible_window() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let (window, pane) = second_window(&mut server);
    let mut out = Vec::new();
    server.handle(Millis(2), Event::Client(ClientId(2), hello()), &mut out);
    server
        .clients
        .get_mut(&ClientId(2))
        .unwrap()
        .view
        .select_window(window);
    server.handle(
        Millis(3),
        Event::Client(
            ClientId(2),
            ClientMsg::Resize(GridSize { cols: 40, rows: 10 }),
        ),
        &mut out,
    );
    assert_eq!(
        server.panes[&PaneId(1)].size,
        GridSize { cols: 20, rows: 4 }
    );
    assert_eq!(server.panes[&pane].size, GridSize { cols: 40, rows: 10 });
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
}

#[test]
fn selected_window_close_prefers_that_clients_last_window() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let (previous, _) = second_window(&mut server);
    let (selected, pane) = second_window(&mut server);
    let view = &mut server.clients.get_mut(&ClientId(1)).unwrap().view;
    view.select_window(previous);
    view.select_window(selected);
    let mut out = Vec::new();
    server.handle(Millis(2), Event::PtyExited(pane), &mut out);
    let final_frame = ack(&mut server, &out);
    ack(&mut server, &final_frame);
    assert_eq!(server.clients[&ClientId(1)].view.window, previous);
    assert_eq!(server.clients[&ClientId(1)].view.last_window, None);
}

#[test]
fn latest_client_disconnect_restores_remaining_clients_size() {
    let (mut server, _) = setup();
    let mut second = hello();
    if let ClientMsg::Hello { size, .. } = &mut second {
        *size = GridSize { cols: 40, rows: 10 };
    }
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), second), &mut out);
    assert_eq!(server.panes[&PaneId(1)].size.cols, 40);
    server.handle(Millis(2), Event::ClientGone(ClientId(2)), &mut out);
    assert_eq!(
        server.panes[&PaneId(1)].size,
        GridSize { cols: 20, rows: 4 }
    );
}

#[test]
fn switching_latest_client_away_restores_remaining_windows_size() {
    let (mut server, _) = setup();
    let mut second = hello();
    if let ClientMsg::Hello { size, .. } = &mut second {
        *size = GridSize { cols: 40, rows: 10 };
    }
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), second), &mut out);
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(2),
            ClientMsg::Command(Command::Switch(Target::Path("/tmp/elsewhere".into()))),
        ),
        &mut out,
    );
    assert_eq!(
        server.panes[&PaneId(1)].size,
        GridSize { cols: 20, rows: 4 }
    );
}

#[test]
fn prefix_detach_restores_remaining_clients_size() {
    let (mut server, _) = setup();
    let mut second = hello();
    if let ClientMsg::Hello { size, .. } = &mut second {
        *size = GridSize { cols: 40, rows: 10 };
    }
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), second), &mut out);
    server.handle(
        Millis(2),
        Event::Client(ClientId(2), key('a', true)),
        &mut out,
    );
    server.handle(
        Millis(3),
        Event::Client(ClientId(2), key('d', false)),
        &mut out,
    );
    assert!(!server.clients.contains_key(&ClientId(2)));
    assert_eq!(
        server.panes[&PaneId(1)].size,
        GridSize { cols: 20, rows: 4 }
    );
}

fn command(server: &mut Server, id: ClientId, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(5),
        Event::Client(id, ClientMsg::Command(command)),
        &mut out,
    );
    out
}

fn order(server: &Server) -> Vec<WindowId> {
    server.tree.sessions[0]
        .windows
        .iter()
        .map(|window| window.id)
        .collect()
}

fn sent_order(out: &[Effect], id: ClientId) -> Option<Vec<(WindowId, u16)>> {
    out.iter().find_map(|effect| {
        let Effect::Send(client, ServerMsg::Status(line)) = effect else {
            return None;
        };
        (*client == id).then(|| {
            line.windows
                .iter()
                .map(|window| (window.id, window.index))
                .collect()
        })
    })
}

fn three_windows() -> Server {
    let (mut server, _) = setup();
    command(&mut server, ClientId(1), Command::NewWindow);
    command(&mut server, ClientId(1), Command::NewWindow);
    let mut out = Vec::new();
    server.handle(Millis(4), Event::Client(ClientId(2), hello()), &mut out);
    assert_eq!(order(&server), [WindowId(1), WindowId(2), WindowId(3)]);
    server
}

fn move_to(window: u32, index: u16) -> Command {
    Command::MoveWindow {
        window: WindowId(window),
        index,
    }
}

#[test]
fn move_window_reorders_and_every_client_sees_the_new_order() {
    let mut server = three_windows();
    let out = command(&mut server, ClientId(1), move_to(3, 1));
    assert_eq!(order(&server), [WindowId(3), WindowId(1), WindowId(2)]);
    let expected = vec![(WindowId(3), 1), (WindowId(1), 2), (WindowId(2), 3)];
    assert_eq!(sent_order(&out, ClientId(1)), Some(expected.clone()));
    assert_eq!(sent_order(&out, ClientId(2)), Some(expected));
    command(&mut server, ClientId(2), move_to(3, 3));
    assert_eq!(order(&server), [WindowId(1), WindowId(2), WindowId(3)]);
    command(&mut server, ClientId(1), move_to(1, 2));
    assert_eq!(order(&server), [WindowId(2), WindowId(1), WindowId(3)]);
}

#[test]
fn move_window_clamps_the_target_index() {
    let mut server = three_windows();
    command(&mut server, ClientId(1), move_to(1, 99));
    assert_eq!(order(&server), [WindowId(2), WindowId(3), WindowId(1)]);
    command(&mut server, ClientId(1), move_to(1, 0));
    assert_eq!(order(&server), [WindowId(1), WindowId(2), WindowId(3)]);
    let out = command(&mut server, ClientId(1), move_to(2, 2));
    assert_eq!(order(&server), [WindowId(1), WindowId(2), WindowId(3)]);
    assert_eq!(sent_order(&out, ClientId(1)), None);
}

#[test]
fn move_window_ignores_a_foreign_or_unknown_window() {
    let mut server = three_windows();
    let (foreign, _) = foreign_window(&mut server);
    for window in [foreign, WindowId(77)] {
        let out = command(
            &mut server,
            ClientId(1),
            Command::MoveWindow { window, index: 1 },
        );
        assert_eq!(order(&server), [WindowId(1), WindowId(2), WindowId(3)]);
        assert_eq!(sent_order(&out, ClientId(1)), None);
    }
    assert_eq!(server.tree.sessions[1].windows[0].id, foreign);
}

fn foreign_window(server: &mut Server) -> (WindowId, PaneId) {
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(99),
            ClientMsg::Control(Control::Create {
                target: Target::Create {
                    name: Some("work".into()),
                    root: "/tmp".into(),
                },
                env: vec![],
            }),
        ),
        &mut out,
    );
    let window = &server.tree.sessions[1].windows[0];
    (window.id, window.active_pane)
}

fn killed(out: &[Effect]) -> Vec<PaneId> {
    out.iter()
        .filter_map(|effect| {
            let Effect::KillPty(pane) = effect else {
                return None;
            };
            Some(*pane)
        })
        .collect()
}

#[test]
fn kill_window_by_id_destroys_only_that_windows_panes() {
    let (mut server, _) = setup();
    command(&mut server, ClientId(1), Command::NewWindow);
    command(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    command(&mut server, ClientId(1), Command::LastWindow);
    assert_eq!(server.tree.sessions[0].windows[1].panes.len(), 2);
    let out = command(&mut server, ClientId(1), Command::KillWindow(WindowId(2)));
    assert_eq!(killed(&out), [PaneId(2), PaneId(3)]);
    assert_eq!(order(&server), [WindowId(1)]);
    assert!(server.panes.contains_key(&PaneId(1)));
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
    assert_eq!(sent_order(&out, ClientId(1)), Some(vec![(WindowId(1), 1)]));
}

#[test]
fn kill_window_of_the_current_window_returns_to_another_window() {
    let (mut server, _) = setup();
    command(&mut server, ClientId(1), Command::NewWindow);
    let out = command(&mut server, ClientId(1), Command::KillWindow(WindowId(2)));
    assert_eq!(killed(&out), [PaneId(2)]);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
}

#[test]
fn kill_window_ignores_a_foreign_or_unknown_window() {
    let (mut server, _) = setup();
    let (foreign, pane) = foreign_window(&mut server);
    for window in [foreign, WindowId(77)] {
        let out = command(&mut server, ClientId(1), Command::KillWindow(window));
        assert!(killed(&out).is_empty());
    }
    assert!(server.panes.contains_key(&pane));
    assert_eq!(server.tree.sessions[1].windows[0].id, foreign);
}
