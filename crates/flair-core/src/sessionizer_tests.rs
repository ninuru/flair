use super::*;
use std::path::Path;

fn session(name: &str, root: &str) -> SessionInfo {
    SessionInfo {
        name: name.into(),
        root: root.into(),
        windows: 1,
    }
}

#[test]
fn open_sessions_first_current_excluded_shorter_path_wins() {
    let items = candidates(
        &[
            session("main", "/home/me"),
            session("running", "/long/running"),
        ],
        "main",
        vec![
            "/short/project".into(),
            "/much/longer/project".into(),
            "/home/me/main".into(),
        ],
    );
    let ranked = rank(&items, "");
    assert_eq!(ranked[0].name, "running");
    assert_eq!(ranked[1].root, PathBuf::from("/short/project"));
    assert_eq!(ranked.len(), 2);
}

#[test]
fn fuzzy_matching_filters_and_matches_noncontiguous_characters() {
    let items = candidates(
        &[],
        "main",
        vec!["/projects/flair".into(), "/projects/other".into()],
    );
    let ranked = rank(&items, "flr");
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].name, "flair");
    assert!(rank(&items, "no-such-project").is_empty());
}

#[test]
fn open_session_wins_name_collision_and_uses_existing_name() {
    let items = candidates(
        &[session("flair", "/existing/flair")],
        "main",
        vec!["/a/flair".into()],
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].target(), Target::Name("flair".into()));
}

#[test]
fn unicode_query_and_non_utf8_paths_are_safe() {
    use std::os::unix::ffi::OsStringExt;
    let items = candidates(
        &[],
        "main",
        vec![
            "/projects/café".into(),
            PathBuf::from(std::ffi::OsString::from_vec(b"/projects/odd-\xff".to_vec())),
        ],
    );
    assert_eq!(rank(&items, "café")[0].name, "café");
    assert_eq!(items.len(), 2);
}

#[test]
fn current_root_is_excluded_even_when_session_name_differs() {
    for current in ["main", "1", "custom"] {
        let items = candidates(
            &[
                session(current, "/projects/foo"),
                session("other", "/projects/other"),
            ],
            current,
            vec!["/projects/foo".into(), "/projects/another".into()],
        );
        assert!(
            items
                .iter()
                .all(|entry| entry.root != Path::new("/projects/foo"))
        );
        assert_eq!(items.len(), 2);
    }
}
