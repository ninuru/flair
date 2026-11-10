use super::*;
use flair_core::{
    command::{PaneRef, WindowRef},
    geom::Axis,
};

fn run(server: &mut Server, id: ClientId, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(10),
        Event::Client(id, ClientMsg::Command(command)),
        &mut out,
    );
    out
}

#[test]
fn split_uses_reported_pane_cwd_but_new_window_uses_session_root() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let root = server.tree.sessions[0].root.clone();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::PtyOutput(
            PaneId(1),
            b"\x1b]7;file://localhost/tmp/split-cwd\x07".to_vec(),
        ),
        &mut out,
    );
    let split = run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    assert!(split.iter().any(|effect| matches!(effect, Effect::Spawn(_, spec) | Effect::SpawnSplit(_, _, spec) if spec.cwd == std::path::Path::new("/tmp/split-cwd"))));
    let window = run(&mut server, ClientId(1), Command::NewWindow);
    assert!(window.iter().any(|effect| matches!(effect, Effect::Spawn(_, spec) | Effect::SpawnSplit(_, _, spec) if spec.cwd == root)));
    assert_eq!(server.tree.sessions[0].windows.len(), 2);
    assert_eq!(server.tree.sessions[0].windows[0].panes.len(), 2);
}

#[test]
fn repeated_split_without_osc_cwd_inherits_its_spawn_cwd() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::PtyOutput(
            PaneId(1),
            b"\x1b]7;file://localhost/tmp/parent\x07".to_vec(),
        ),
        &mut out,
    );
    run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let split = run(&mut server, ClientId(1), Command::Split(Axis::Vertical));
    assert!(split.iter().any(|effect| matches!(effect, Effect::Spawn(_, spec) | Effect::SpawnSplit(_, _, spec) if spec.cwd == std::path::Path::new("/tmp/parent"))));
}

#[test]
fn new_window_changes_only_requesting_clients_view_and_last_window() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), hello()), &mut out);
    run(&mut server, ClientId(1), Command::NewWindow);
    let new_window = server.clients[&ClientId(1)].view.window;
    assert_ne!(new_window, WindowId(1));
    assert_eq!(
        server.clients[&ClientId(1)].view.last_window,
        Some(WindowId(1))
    );
    assert_eq!(server.clients[&ClientId(2)].view.window, WindowId(1));
    run(&mut server, ClientId(1), Command::LastWindow);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
    assert_eq!(
        server.clients[&ClientId(1)].view.last_window,
        Some(new_window)
    );
}

#[test]
fn close_window_renumbers_and_select_window_uses_one_based_index() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::NewWindow);
    run(&mut server, ClientId(1), Command::NewWindow);
    let third = server.clients[&ClientId(1)].view.window;
    run(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Index(2)),
    );
    run(&mut server, ClientId(1), Command::CloseWindow);
    assert_eq!(server.tree.sessions[0].windows.len(), 2);
    assert_eq!(server.tree.sessions[0].window_number(third), Some(2));
    run(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Index(2)),
    );
    assert_eq!(server.clients[&ClientId(1)].view.window, third);
}

#[test]
fn close_last_window_returns_each_client_to_their_previous_session() {
    let (mut server, _) = setup();
    run(
        &mut server,
        ClientId(1),
        Command::Switch(Target::Path("/tmp/other".into())),
    );
    let out = run(&mut server, ClientId(1), Command::CloseWindow);
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(1));
    assert_eq!(server.tree.sessions.len(), 1);
    assert!(!out.iter().any(|effect| matches!(effect, Effect::Shutdown)));
}

#[test]
fn unknown_pane_or_foreign_window_cannot_change_view() {
    let (mut server, _) = setup();
    run(
        &mut server,
        ClientId(1),
        Command::SelectPane(PaneRef::Id(PaneId(999))),
    );
    run(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Id(WindowId(999))),
    );
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(1));
    assert_eq!(server.client_pane(ClientId(1)), Some(PaneId(1)));
}

#[test]
fn split_frames_include_all_visible_panes_and_inactive_output() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let split = run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let window = server.tree.window(WindowId(1)).unwrap();
    let second = window.active_pane;
    let layout = window.geometry(server.band());
    assert_eq!(layout.panes.len(), 2);
    assert_eq!(layout.panes[0].rect.origin, CellPoint { col: 0, row: 0 });
    assert_eq!(server.panes[&PaneId(1)].size, GridSize { cols: 9, rows: 4 });
    assert_eq!(server.panes[&second].size, GridSize { cols: 10, rows: 4 });
    assert!(split.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Frame {panes,..}) if panes.len()==2 && panes.iter().all(|pane| pane.full))));
    ack(&mut server, &split);
    let mut output = Vec::new();
    server.handle(
        Millis(11),
        Event::PtyOutput(PaneId(1), b"inactive".to_vec()),
        &mut output,
    );
    assert!(output.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame {panes,..}) if panes.iter().any(|pane| pane.pane == PaneId(1) && pane.rows[0].1[0].c == 'i'))));
    assert_eq!(server.client_pane(ClientId(1)), Some(second));
}

#[test]
fn latest_active_client_resizes_every_split_pane() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let second_pane = server.client_pane(ClientId(1)).unwrap();
    let mut second = hello();
    if let ClientMsg::Hello { size, .. } = &mut second {
        *size = GridSize { cols: 41, rows: 10 };
    }
    let mut out = Vec::new();
    server.handle(Millis(11), Event::Client(ClientId(2), second), &mut out);
    for pane in [PaneId(1), second_pane] {
        assert_eq!(server.panes[&pane].size, GridSize { cols: 20, rows: 10 });
    }
    server.handle(
        Millis(12),
        Event::Client(ClientId(1), ClientMsg::Focus(true)),
        &mut out,
    );
    assert_eq!(server.panes[&PaneId(1)].size, GridSize { cols: 9, rows: 4 });
    assert_eq!(
        server.panes[&second_pane].size,
        GridSize { cols: 10, rows: 4 }
    );
}

#[test]
fn closing_a_split_pane_collapses_layout_and_resizes_survivor() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let out = run(&mut server, ClientId(1), Command::ClosePane);
    assert_eq!(
        server.tree.window(WindowId(1)).unwrap().panes,
        vec![PaneId(1)]
    );
    assert_eq!(
        server.panes[&PaneId(1)].size,
        GridSize { cols: 20, rows: 4 }
    );
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(PaneId(2))))
    );
}

#[test]
fn zoom_is_shared_and_restores_split_geometry() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let before = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry(server.band());
    run(&mut server, ClientId(1), Command::ToggleZoom);
    let zoomed = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry(server.band());
    assert!(zoomed.zoomed);
    assert_eq!(zoomed.panes.len(), 1);
    assert_eq!(zoomed.panes[0].pane, before.active);
    assert!(zoomed.separators.is_empty());
    assert_eq!(
        server.panes[&before.active].size,
        GridSize { cols: 20, rows: 4 }
    );
    run(&mut server, ClientId(1), Command::ToggleZoom);
    assert_eq!(
        server
            .tree
            .window(WindowId(1))
            .unwrap()
            .geometry(server.band()),
        before
    );
}

#[test]
fn final_window_close_uses_each_clients_own_session_history() {
    let (mut server, _) = setup();
    run(
        &mut server,
        ClientId(1),
        Command::Switch(Target::Path("/tmp/shared".into())),
    );
    let shared = server.clients[&ClientId(1)].view.session;
    let mut second = hello();
    if let ClientMsg::Hello { target, .. } = &mut second {
        *target = Target::Path("/tmp/private".into());
    }
    let mut out = Vec::new();
    server.handle(Millis(11), Event::Client(ClientId(2), second), &mut out);
    let private = server.clients[&ClientId(2)].view.session;
    run(
        &mut server,
        ClientId(2),
        Command::Switch(Target::Name("shared".into())),
    );
    assert_eq!(server.clients[&ClientId(2)].view.session, shared);
    run(&mut server, ClientId(1), Command::CloseWindow);
    assert_eq!(server.clients[&ClientId(1)].view.session, SessionId(1));
    assert_eq!(server.clients[&ClientId(2)].view.session, private);
}

#[test]
fn active_split_selection_converts_full_grid_coordinates() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    let pane = server.client_pane(ClientId(1)).unwrap();
    let origin = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry(server.band())
        .panes[1]
        .rect
        .origin;
    let mut out = Vec::new();
    server.handle(
        Millis(11),
        Event::PtyOutput(pane, b"hello".to_vec()),
        &mut out,
    );
    for (offset, action) in [(0, MouseAction::Press), (4, MouseAction::Release)] {
        server.handle(
            Millis(12),
            Event::Client(
                ClientId(1),
                ClientMsg::Input(InputEvent::Mouse(flair_core::key::MouseInput {
                    point: CellPoint {
                        col: origin.col + offset,
                        row: origin.row,
                    },
                    button: MouseButton::Left,
                    action,
                    mods: Mods::default(),
                })),
            ),
            &mut out,
        );
    }
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::SetClipboard {text,primary:true}) if text=="hello")));
}

#[test]
fn too_small_split_leaves_window_and_id_allocation_unchanged() {
    let (mut server, _) = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(11),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 2, rows: 4 }),
        ),
        &mut out,
    );
    let before = server.tree.clone();
    let next_id = server.next_id;
    let out = run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    assert_eq!(server.tree, before);
    assert_eq!(server.next_id, next_id);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Error(_))))
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::Spawn(_, _) | Effect::SpawnSplit(_, _, _)))
    );
}

#[test]
fn no_history_windows_and_splits_preserve_environment_policy() {
    let (mut server, _) = setup();
    run(
        &mut server,
        ClientId(1),
        Command::Switch(Target::Create {
            name: Some("H".into()),
            root: "/tmp".into(),
        }),
    );
    for command in [Command::NewWindow, Command::Split(Axis::Horizontal)] {
        let out = run(&mut server, ClientId(1), command);
        let spec = out
            .iter()
            .find_map(|effect| match effect {
                Effect::Spawn(_, spec) | Effect::SpawnSplit(_, _, spec) => Some(spec),
                Effect::WritePty(_, _)
                | Effect::ResizePty(_, _)
                | Effect::KillPty(_)
                | Effect::Send(_, _)
                | Effect::SetTimer(_, _)
                | Effect::ReadConfig(_)
                | Effect::Shutdown => None,
            })
            .unwrap();
        assert!(spec.unset_env.iter().any(|key| key == "HISTFILE"));
        assert!(!spec.env.iter().any(|(key, _)| key == "HISTFILE"));
        assert!(spec.env.iter().any(|(key, _)| key == "FLAIR_PANE"));
    }
}

#[test]
fn exited_pane_is_reaped_before_sibling_refills_frame_credit() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let split = run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    ack(&mut server, &split);
    let exited = server.client_pane(ClientId(1)).unwrap();
    let mut final_frame = Vec::new();
    server.handle(
        Millis(11),
        Event::PtyOutput(exited, b"final".to_vec()),
        &mut final_frame,
    );
    let mut out = Vec::new();
    server.handle(Millis(12), Event::PtyExited(exited), &mut out);
    server.handle(
        Millis(13),
        Event::PtyOutput(PaneId(1), b"busy sibling".to_vec()),
        &mut out,
    );
    let effects = ack(&mut server, &final_frame);
    assert!(!server.panes.contains_key(&exited));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::KillPty(pane) if *pane == exited))
    );
}

#[test]
fn closing_windows_without_valid_history_chooses_adjacent_survivor() {
    let (mut server, _) = setup();
    for _ in 0..3 {
        run(&mut server, ClientId(1), Command::NewWindow);
    }
    run(&mut server, ClientId(1), Command::CloseWindow);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(3));
    run(&mut server, ClientId(1), Command::CloseWindow);
    assert_eq!(server.clients[&ClientId(1)].view.window, WindowId(2));
}

#[test]
fn fresh_attach_restores_session_most_recently_shown_window() {
    let (mut server, _) = setup();
    run(&mut server, ClientId(1), Command::NewWindow);
    let selected = server.clients[&ClientId(1)].view.window;
    let mut out = Vec::new();
    server.handle(
        Millis(11),
        Event::Client(ClientId(1), ClientMsg::Detach),
        &mut out,
    );
    server.handle(Millis(12), Event::Client(ClientId(2), hello()), &mut out);
    assert_eq!(server.clients[&ClientId(2)].view.window, selected);
    run(&mut server, ClientId(2), Command::CloseWindow);
    server.handle(Millis(13), Event::Client(ClientId(3), hello()), &mut out);
    assert_eq!(server.clients[&ClientId(3)].view.window, WindowId(1));
}

#[test]
fn reattach_after_close_restores_actual_client_fallback() {
    let (mut server, _) = setup();
    for _ in 0..3 {
        run(&mut server, ClientId(1), Command::NewWindow);
    }
    run(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Index(2)),
    );
    run(
        &mut server,
        ClientId(1),
        Command::SelectWindow(WindowRef::Index(4)),
    );
    run(&mut server, ClientId(1), Command::CloseWindow);
    let mut out = Vec::new();
    server.handle(
        Millis(11),
        Event::Client(ClientId(1), ClientMsg::Detach),
        &mut out,
    );
    server.handle(Millis(12), Event::Client(ClientId(2), hello()), &mut out);
    assert_eq!(server.clients[&ClientId(2)].view.window, WindowId(2));
}

#[test]
fn exited_pane_reaps_after_staggered_clients_acknowledge_its_final_snapshot() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let split = run(&mut server, ClientId(1), Command::Split(Axis::Horizontal));
    ack(&mut server, &split);
    let exited = server.client_pane(ClientId(1)).unwrap();
    let mut attached = Vec::new();
    server.handle(
        Millis(11),
        Event::Client(ClientId(2), hello()),
        &mut attached,
    );
    ack(&mut server, &attached);
    let mut final_frames = Vec::new();
    server.handle(
        Millis(12),
        Event::PtyOutput(exited, b"final".to_vec()),
        &mut final_frames,
    );
    let mut out = Vec::new();
    server.handle(Millis(13), Event::PtyExited(exited), &mut out);
    server.handle(
        Millis(14),
        Event::PtyOutput(PaneId(1), b"sibling".to_vec()),
        &mut out,
    );
    let sequence = |id| {
        final_frames
            .iter()
            .find_map(|effect| match effect {
                Effect::Send(client, ServerMsg::Frame { seq, .. }) if *client == id => Some(*seq),
                Effect::Spawn(_, _)
                | Effect::SpawnSplit(_, _, _)
                | Effect::WritePty(_, _)
                | Effect::ResizePty(_, _)
                | Effect::KillPty(_)
                | Effect::Send(_, _)
                | Effect::SetTimer(_, _)
                | Effect::ReadConfig(_)
                | Effect::Shutdown => None,
            })
            .unwrap()
    };
    server.handle(
        Millis(15),
        Event::Client(
            ClientId(1),
            ClientMsg::FrameAck {
                seq: sequence(ClientId(1)),
            },
        ),
        &mut out,
    );
    assert!(server.clients[&ClientId(1)].awaiting_ack.is_some());
    assert!(server.panes.contains_key(&exited));
    server.handle(
        Millis(16),
        Event::PtyOutput(PaneId(1), b"more".to_vec()),
        &mut out,
    );
    server.handle(
        Millis(17),
        Event::Client(
            ClientId(2),
            ClientMsg::FrameAck {
                seq: sequence(ClientId(2)),
            },
        ),
        &mut out,
    );
    assert!(!server.panes.contains_key(&exited));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(pane) if *pane==exited))
    );
}

#[test]
fn restored_final_content_must_be_sent_and_acknowledged_after_intermediate_frame() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let mut first = Vec::new();
    server.handle(
        Millis(11),
        Event::PtyOutput(PaneId(1), b"A\r".to_vec()),
        &mut first,
    );
    ack(&mut server, &first);
    let mut intermediate = Vec::new();
    server.handle(
        Millis(12),
        Event::PtyOutput(PaneId(1), b"B\r".to_vec()),
        &mut intermediate,
    );
    let mut out = Vec::new();
    server.handle(
        Millis(13),
        Event::PtyOutput(PaneId(1), b"A\r".to_vec()),
        &mut out,
    );
    server.handle(Millis(14), Event::PtyExited(PaneId(1)), &mut out);
    assert!(server.panes.contains_key(&PaneId(1)));
    let restored = ack(&mut server, &intermediate);
    assert!(restored.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::Frame {panes,..}) if panes[0].rows[0].1[0].c=='A')));
    assert!(server.panes.contains_key(&PaneId(1)));
    let final_ack = ack(&mut server, &restored);
    assert!(
        final_ack
            .iter()
            .any(|effect| matches!(effect, Effect::KillPty(PaneId(1))))
    );
    assert!(!server.panes.contains_key(&PaneId(1)));
}

fn prefixed(server: &mut Server, c: char, unshifted: char) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(10),
        Event::Client(
            ClientId(1),
            ClientMsg::Resize(GridSize { cols: 20, rows: 10 }),
        ),
        &mut out,
    );
    server.handle(
        Millis(10),
        Event::PtyOutput(
            PaneId(1),
            b"\x1b]7;file://localhost/tmp/split-cwd\x07".to_vec(),
        ),
        &mut out,
    );
    for (key, mods, text, unshifted) in [
        (
            Key::Char('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            None,
            Some('a'),
        ),
        (
            Key::Char(c),
            Mods {
                shift: true,
                ..Mods::default()
            },
            Some(c.to_string()),
            Some(unshifted),
        ),
    ] {
        out.clear();
        server.handle(
            Millis(11),
            Event::Client(
                ClientId(1),
                ClientMsg::Input(InputEvent::Key(KeyInput {
                    key,
                    mods,
                    text,
                    repeat: false,
                    keypad: false,
                    action: KeyAction::Press,
                    unshifted,
                    shifted: None,
                    base: unshifted,
                    locks: Locks::default(),
                })),
            ),
            &mut out,
        );
    }
    out
}

fn assert_split_in_pane_cwd(out: &[Effect]) {
    assert!(out.iter().any(|effect| matches!(effect, Effect::Spawn(_, spec) | Effect::SpawnSplit(_, _, spec) if spec.cwd == std::path::Path::new("/tmp/split-cwd"))));
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::WritePty(_, _)))
    );
}

#[test]
fn prefix_quote_stacks_panes_top_and_bottom() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let out = prefixed(&mut server, '"', '\'');
    assert_split_in_pane_cwd(&out);
    let layout = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry(server.band());
    let [top, bottom] = layout.panes.as_slice() else {
        panic!("expected two panes, got {:?}", layout.panes);
    };
    assert_eq!(top.rect.origin.col, bottom.rect.origin.col);
    assert_eq!(top.rect.size.cols, bottom.rect.size.cols);
    assert!(top.rect.origin.row + top.rect.size.rows <= bottom.rect.origin.row);
    let [divider] = layout.separators.as_slice() else {
        panic!("expected one divider, got {:?}", layout.separators);
    };
    assert_eq!(divider.rect.size.rows, 1);
    assert_eq!(divider.rect.size.cols, top.rect.size.cols);
}

#[test]
fn prefix_percent_places_panes_left_and_right() {
    let (mut server, initial) = setup();
    ack(&mut server, &initial);
    let out = prefixed(&mut server, '%', '5');
    assert_split_in_pane_cwd(&out);
    let layout = server
        .tree
        .window(WindowId(1))
        .unwrap()
        .geometry(server.band());
    let [left, right] = layout.panes.as_slice() else {
        panic!("expected two panes, got {:?}", layout.panes);
    };
    assert_eq!(left.rect.origin.row, right.rect.origin.row);
    assert_eq!(left.rect.size.rows, right.rect.size.rows);
    assert!(left.rect.origin.col + left.rect.size.cols <= right.rect.origin.col);
    let [divider] = layout.separators.as_slice() else {
        panic!("expected one divider, got {:?}", layout.separators);
    };
    assert_eq!(divider.rect.size.cols, 1);
    assert_eq!(divider.rect.size.rows, left.rect.size.rows);
}
