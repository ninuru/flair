use super::*;

fn single() -> Tree {
    Tree::single(
        SessionId(0),
        "main".to_owned(),
        PathBuf::from("/home/u"),
        WindowId(0),
        PaneId(0),
    )
}

#[test]
fn single_has_one_session_window_and_pane() {
    let tree = single();
    let session = tree.session(SessionId(0)).unwrap();
    assert_eq!(session.name, "main");
    assert_eq!(session.root, PathBuf::from("/home/u"));
    assert_eq!(session.windows.len(), 1);
    assert_eq!(session.windows[0].panes, vec![PaneId(0)]);
    assert!(tree.session(SessionId(1)).is_none());
}

#[test]
fn active_pane_of_window() {
    let tree = single();
    assert_eq!(tree.active_pane(WindowId(0)), Some(PaneId(0)));
    assert_eq!(tree.active_pane(WindowId(1)), None);
}

#[test]
fn removing_the_last_pane_empties_the_tree() {
    let mut tree = single();
    assert_eq!(
        tree.remove_pane(PaneId(0)),
        Removed {
            window: Some(WindowId(0)),
            session: Some(SessionId(0)),
        }
    );
    assert!(tree.sessions.is_empty());
}

#[test]
fn removing_an_unknown_pane_keeps_the_tree() {
    let mut tree = single();
    assert_eq!(tree.remove_pane(PaneId(9)), Removed::default());
    assert_eq!(tree, single());
}

#[test]
fn removing_the_active_pane_activates_a_neighbour() {
    let mut tree = single();
    tree.sessions[0].windows[0].panes = vec![PaneId(0), PaneId(1), PaneId(2)];
    tree.sessions[0].windows[0].active_pane = PaneId(2);
    assert_eq!(tree.remove_pane(PaneId(2)), Removed::default());
    assert_eq!(tree.active_pane(WindowId(0)), Some(PaneId(1)));
    tree.sessions[0].windows[0].active_pane = PaneId(0);
    assert_eq!(tree.remove_pane(PaneId(0)), Removed::default());
    assert_eq!(tree.active_pane(WindowId(0)), Some(PaneId(1)));
}

#[test]
fn removing_a_window_s_last_pane_keeps_the_session() {
    let mut tree = single();
    tree.sessions[0].windows.push(Window {
        id: WindowId(1),
        panes: vec![PaneId(1)],
        active_pane: PaneId(1),
    });
    assert_eq!(
        tree.remove_pane(PaneId(0)),
        Removed {
            window: Some(WindowId(0)),
            session: None,
        }
    );
    let session = tree.session(SessionId(0)).unwrap();
    assert_eq!(session.windows.len(), 1);
    assert_eq!(session.windows[0].id, WindowId(1));
}
