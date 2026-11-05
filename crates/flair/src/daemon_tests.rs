use super::*;
use flair_core::ids::PaneId;

#[test]
fn pane_routing_is_scoped_to_the_selected_daemon() {
    assert_eq!(
        matching_pane(
            Path::new("/tmp/dev.sock"),
            Some("/tmp/regular.sock,1234"),
            Some("%103")
        ),
        None
    );
    assert_eq!(
        matching_pane(
            Path::new("/tmp/regular.sock"),
            Some("/tmp/regular.sock,1234"),
            Some("%103")
        ),
        Some(PaneId(103))
    );
    assert_eq!(
        matching_pane(Path::new("/tmp/regular.sock"), None, Some("%103")),
        None
    );
    assert_eq!(
        matching_pane(
            Path::new("/tmp/regular.sock"),
            Some("/tmp/regular.sock,1234"),
            Some("invalid")
        ),
        None
    );
}
