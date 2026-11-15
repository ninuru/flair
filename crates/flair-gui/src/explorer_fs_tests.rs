use super::*;
use std::{
    fs::{DirBuilder, File, Permissions},
    os::unix::fs::{DirBuilderExt, PermissionsExt, symlink},
    path::PathBuf,
    process,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("flair-explorer-{}-{index}", process::id()));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        TempDir(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }

    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        File::create(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn names(listing: &Listing) -> Vec<(String, bool)> {
    listing
        .entries
        .iter()
        .map(|entry| (entry.name.to_string_lossy().into_owned(), entry.dir))
        .collect()
}

#[test]
fn folders_come_first_then_names_case_insensitively() {
    let temp = TempDir::new();
    for name in ["b.txt", "A.txt", "a.txt", ".hidden", "Zeta"] {
        temp.file(name);
    }
    for name in ["src", "Docs", ".git"] {
        temp.dir(name);
    }
    let listing = read_listing(&temp.0);
    assert_eq!(listing.dir, temp.0);
    assert_eq!(listing.error, None);
    assert_eq!(listing.more, 0);
    assert_eq!(
        names(&listing),
        [
            (".git".to_string(), true),
            ("Docs".to_string(), true),
            ("src".to_string(), true),
            (".hidden".to_string(), false),
            ("A.txt".to_string(), false),
            ("a.txt".to_string(), false),
            ("b.txt".to_string(), false),
            ("Zeta".to_string(), false),
        ]
    );
}

#[test]
fn a_symlinked_folder_is_a_folder_and_a_dangling_link_is_a_file() {
    let temp = TempDir::new();
    let target = temp.dir("target");
    symlink(&target, temp.0.join("link")).unwrap();
    symlink(temp.0.join("missing"), temp.0.join("dangling")).unwrap();
    let listing = read_listing(&temp.0);
    assert_eq!(
        names(&listing),
        [
            ("link".to_string(), true),
            ("target".to_string(), true),
            ("dangling".to_string(), false),
        ]
    );
}

#[test]
fn a_large_directory_lists_every_entry() {
    let temp = TempDir::new();
    for index in 0..1005 {
        temp.file(&format!("f{index:05}"));
    }
    let listing = read_listing(&temp.0);
    assert_eq!(listing.entries.len(), 1005);
    assert_eq!(listing.more, 0);
    assert_eq!(listing.entries[0].name, "f00000");
    assert_eq!(listing.entries[1004].name, "f01004");
}

#[test]
fn a_missing_directory_becomes_an_error() {
    let temp = TempDir::new();
    let listing = read_listing(&temp.0.join("gone"));
    assert!(listing.entries.is_empty());
    assert!(listing.error.is_some());
}

#[test]
fn an_unreadable_directory_becomes_an_error() {
    if unsafe { libc::geteuid() } == 0 {
        log::info!("skipping: root can read every directory");
        return;
    }
    let temp = TempDir::new();
    let locked = temp.dir("locked");
    temp.file("locked/secret");
    fs::set_permissions(&locked, Permissions::from_mode(0o000)).unwrap();
    let listing = read_listing(&locked);
    fs::set_permissions(&locked, Permissions::from_mode(0o700)).unwrap();
    assert!(listing.entries.is_empty());
    assert!(
        listing
            .error
            .as_deref()
            .is_some_and(|error| error.contains("ermission denied")),
        "{listing:?}"
    );
}

#[test]
fn the_worker_delivers_listings_through_the_callback() {
    let temp = TempDir::new();
    temp.file("one");
    let (tx, rx) = mpsc::channel();
    let worker = FsWorker::spawn(move |listing| {
        let _ = tx.send(listing);
    })
    .unwrap();
    worker.request(temp.0.clone());
    worker.request(temp.0.join("gone"));
    let first = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(first.dir, temp.0);
    assert_eq!(names(&first), [("one".to_string(), false)]);
    let second = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(second.error.is_some());
    drop(worker);
    assert!(rx.recv_timeout(Duration::from_secs(10)).is_err());
}
