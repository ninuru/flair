use super::*;

#[test]
fn counters_are_sequential_and_independent() {
    let mut ids = IdGen::default();
    assert_eq!(ids.next_pane(), PaneId(0));
    assert_eq!(ids.next_pane(), PaneId(1));
    assert_eq!(ids.next_window(), WindowId(0));
    assert_eq!(ids.next_session(), SessionId(0));
    assert_eq!(ids.next_client(), ClientId(0));
    assert_eq!(ids.next_pane(), PaneId(2));
}
