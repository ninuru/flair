use super::*;

const SIZE_A: GridSize = GridSize { cols: 80, rows: 24 };
const SIZE_B: GridSize = GridSize {
    cols: 120,
    rows: 40,
};

#[test]
fn new_view_has_no_history() {
    let view = ClientView::new(SessionId(0), WindowId(0), Millis(5));
    assert_eq!(view.last_window, None);
    assert_eq!(view.last_session, None);
    assert_eq!(view.last_active, Millis(5));
}

#[test]
fn select_window_sets_last_window() {
    let mut view = ClientView::new(SessionId(0), WindowId(0), Millis(0));
    view.select_window(WindowId(1));
    assert_eq!(view.window, WindowId(1));
    assert_eq!(view.last_window, Some(WindowId(0)));
    view.select_window(WindowId(1));
    assert_eq!(view.last_window, Some(WindowId(0)));
}

#[test]
fn switch_session_sets_last_session() {
    let mut view = ClientView::new(SessionId(0), WindowId(0), Millis(0));
    view.select_window(WindowId(1));
    view.switch_session(SessionId(1), WindowId(5));
    assert_eq!(view.session, SessionId(1));
    assert_eq!(view.window, WindowId(5));
    assert_eq!(view.last_session, Some(SessionId(0)));
    assert_eq!(view.last_window, None);
}

#[test]
fn switch_to_same_session_selects_window() {
    let mut view = ClientView::new(SessionId(0), WindowId(0), Millis(0));
    view.switch_session(SessionId(0), WindowId(1));
    assert_eq!(view.last_session, None);
    assert_eq!(view.last_window, Some(WindowId(0)));
}

#[test]
fn switch_to_current_session_and_window_keeps_history() {
    let mut view = ClientView::new(SessionId(0), WindowId(0), Millis(0));
    view.switch_session(SessionId(1), WindowId(5));
    view.select_window(WindowId(6));
    view.switch_session(SessionId(1), WindowId(6));
    assert_eq!(view.session, SessionId(1));
    assert_eq!(view.window, WindowId(6));
    assert_eq!(view.last_session, Some(SessionId(0)));
    assert_eq!(view.last_window, Some(WindowId(5)));
}

#[test]
fn window_size_follows_latest_active_client() {
    let mut a = ClientView::new(SessionId(0), WindowId(0), Millis(10));
    let b = ClientView::new(SessionId(0), WindowId(0), Millis(20));
    assert_eq!(
        window_size(
            [(ClientId(0), &a, SIZE_A), (ClientId(1), &b, SIZE_B)].into_iter(),
            WindowId(0)
        ),
        Some(SIZE_B)
    );
    a.touch(Millis(30));
    assert_eq!(
        window_size(
            [(ClientId(0), &a, SIZE_A), (ClientId(1), &b, SIZE_B)].into_iter(),
            WindowId(0)
        ),
        Some(SIZE_A)
    );
}

#[test]
fn window_size_ignores_clients_on_other_windows() {
    let a = ClientView::new(SessionId(0), WindowId(0), Millis(10));
    let b = ClientView::new(SessionId(0), WindowId(1), Millis(20));
    let clients = [(ClientId(0), &a, SIZE_A), (ClientId(1), &b, SIZE_B)];
    assert_eq!(window_size(clients.into_iter(), WindowId(0)), Some(SIZE_A));
    assert_eq!(window_size(clients.into_iter(), WindowId(2)), None);
}

#[test]
fn window_size_tie_goes_to_lower_client_id() {
    let a = ClientView::new(SessionId(0), WindowId(0), Millis(10));
    let b = ClientView::new(SessionId(0), WindowId(0), Millis(10));
    let forward = [(ClientId(3), &a, SIZE_A), (ClientId(7), &b, SIZE_B)];
    let backward = [(ClientId(7), &b, SIZE_B), (ClientId(3), &a, SIZE_A)];
    assert_eq!(window_size(forward.into_iter(), WindowId(0)), Some(SIZE_A));
    assert_eq!(window_size(backward.into_iter(), WindowId(0)), Some(SIZE_A));
}
