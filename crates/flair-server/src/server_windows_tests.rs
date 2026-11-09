use super::*;

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
