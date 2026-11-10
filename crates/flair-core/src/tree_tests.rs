use super::*;
use crate::geom::Axis;

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
    tree.sessions[0].windows.push(Window::new(
        WindowId(1),
        PaneId(1),
        GridSize { cols: 80, rows: 25 },
    ));
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

#[test]
fn window_numbers_start_at_one_and_close_gaps() {
    let mut tree = single();
    for number in [7, 12] {
        tree.sessions[0].windows.push(Window::new(
            WindowId(number),
            PaneId(number),
            GridSize { cols: 80, rows: 25 },
        ));
    }
    assert_eq!(tree.sessions[0].window_number(WindowId(0)), Some(1));
    assert_eq!(tree.sessions[0].window_number(WindowId(7)), Some(2));
    assert_eq!(tree.sessions[0].window_number(WindowId(12)), Some(3));
    tree.remove_pane(PaneId(7));
    assert_eq!(tree.sessions[0].window_number(WindowId(12)), Some(2));
    assert_eq!(tree.sessions[0].window_number(WindowId(7)), None);
}

#[test]
fn window_area_starts_at_row_zero_and_covers_the_whole_size() {
    let size = GridSize { cols: 80, rows: 24 };
    let window = Window::new(WindowId(3), PaneId(3), size);
    let area = CellRect {
        origin: CellPoint { col: 0, row: 0 },
        size,
    };
    assert_eq!(window.area(), area);
    let geometry = window.geometry(0);
    assert_eq!(geometry.area, area);
    assert_eq!(geometry.panes[0].rect, area);
}

#[test]
fn window_geometry_applies_the_band_to_every_leaf() {
    let size = GridSize { cols: 80, rows: 24 };
    let window = Window::new(WindowId(3), PaneId(3), size);
    let geometry = window.geometry(1);
    assert_eq!(geometry.panes[0].band, 1);
    assert_eq!(geometry.panes[0].rect.origin, CellPoint { col: 0, row: 1 });
    assert_eq!(geometry.panes[0].rect.size, GridSize { cols: 80, rows: 23 });
    assert_eq!(geometry.panes[0].outer(), window.area());
}

#[test]
fn a_zoomed_window_applies_the_band_to_the_active_pane() {
    let size = GridSize { cols: 80, rows: 24 };
    let mut window = Window::new(WindowId(3), PaneId(3), size);
    assert!(
        window
            .layout
            .split(PaneId(3), PaneId(4), Axis::Vertical, window.area())
    );
    window.panes.push(PaneId(4));
    window.active_pane = PaneId(4);
    window.zoomed = true;
    let geometry = window.geometry(1);
    assert_eq!(geometry.panes.len(), 1);
    assert_eq!(geometry.panes[0].pane, PaneId(4));
    assert_eq!(geometry.panes[0].band, 1);
    assert_eq!(geometry.panes[0].outer(), window.area());
    assert_eq!(window.geometry(0).panes[0].rect, window.area());
}
