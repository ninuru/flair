use super::*;
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{self, Command},
};

pub(crate) fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub(crate) fn temp(tag: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("flair-git-{tag}-{}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

pub(crate) fn run(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=flair",
            "-c",
            "user.email=flair",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub(crate) fn commit(dir: &Path, file: &str, text: &str) {
    fs::create_dir_all(dir).unwrap();
    if !dir.join(".git").exists() {
        run(dir, &["init", "-q", "-b", "main"]);
    }
    fs::write(dir.join(file), text).unwrap();
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", file]);
}

fn outcome(dir: &Path, changes: bool, deadline: Duration) -> Read {
    let (root, _) = open(dir).unwrap();
    let (read, straggler) = read(root, changes, deadline);
    if let Some(straggler) = straggler {
        straggler.join().unwrap();
    }
    read
}

fn info(dir: &Path, changes: bool, deadline: Duration) -> GitInfo {
    match outcome(dir, changes, deadline) {
        Read::Done(info) => info,
        other @ (Read::Slow(_) | Read::Failed(_)) => panic!("unfinished read: {other:?}"),
    }
}

fn none() -> Changes {
    Changes {
        files: 0,
        added: 0,
        removed: 0,
    }
}

#[test]
fn a_directory_outside_any_repository_has_no_git_facts() {
    let dir = temp("plain");
    assert!(open(&dir).is_none());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_fresh_commit_shows_its_branch_and_a_clean_tree() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("clean");
    commit(&dir, "a.txt", "one\ntwo\n");
    fs::create_dir_all(dir.join("sub")).unwrap();
    let (root, _) = open(&dir.join("sub")).unwrap();
    assert_eq!(root, dir);
    let found = info(&dir, true, Duration::from_secs(2));
    assert_eq!(found.head, Head::Branch("main".into()));
    assert_eq!(found.upstream, None);
    assert_eq!(found.changes, Some(none()));
    assert_eq!(info(&dir, false, Duration::from_secs(2)).changes, None);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changes_count_files_with_untracked_ones_and_lines_against_head() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("dirty");
    commit(&dir, "a.txt", "one\ntwo\n");
    fs::write(dir.join("a.txt"), "one\nthree\n").unwrap();
    fs::write(dir.join("new.txt"), "x\ny\n").unwrap();
    let found = info(&dir, true, Duration::from_secs(2));
    assert_eq!(
        found.changes,
        Some(Changes {
            files: 2,
            added: 1,
            removed: 1
        })
    );
    fs::write(dir.join("staged.txt"), "s\n").unwrap();
    run(&dir, &["add", "staged.txt"]);
    fs::remove_file(dir.join("new.txt")).unwrap();
    let found = info(&dir, true, Duration::from_secs(2));
    assert_eq!(
        found.changes,
        Some(Changes {
            files: 2,
            added: 2,
            removed: 1
        })
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_detached_head_shows_the_short_commit() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("detached");
    commit(&dir, "a.txt", "one\n");
    run(&dir, &["checkout", "-q", "--detach"]);
    let short = run(&dir, &["rev-parse", "--short=7", "HEAD"]);
    assert_eq!(short.len(), 7);
    assert_eq!(
        info(&dir, true, Duration::from_secs(2)).head,
        Head::Detached(short)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ahead_and_behind_count_against_the_upstream() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("upstream");
    let bare = dir.join("up.git");
    let work = dir.join("work");
    fs::create_dir_all(&bare).unwrap();
    run(&bare, &["init", "-q", "--bare", "-b", "main"]);
    commit(&work, "a.txt", "one\n");
    run(&work, &["remote", "add", "origin", bare.to_str().unwrap()]);
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    commit(&work, "b.txt", "two\n");
    let ahead = AheadBehind {
        ahead: 1,
        behind: 0,
    };
    assert_eq!(
        info(&work, false, Duration::from_secs(2)).upstream,
        Some(ahead)
    );
    run(&work, &["push", "-q"]);
    run(&work, &["checkout", "-q", "-B", "main", "HEAD~1"]);
    let behind = AheadBehind {
        ahead: 0,
        behind: 1,
    };
    assert_eq!(
        info(&work, false, Duration::from_secs(2)).upstream,
        Some(behind)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_status_past_its_deadline_leaves_changes_unknown() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("deadline");
    commit(&dir, "a.txt", "one\n");
    match outcome(&dir, true, Duration::ZERO) {
        Read::Slow(found) => assert!(found.is_none_or(|found| found.changes.is_none())),
        other @ (Read::Done(_) | Read::Failed(_)) => panic!("not slow: {other:?}"),
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_broken_status_leaves_changes_unknown_without_being_slow() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("broken");
    commit(&dir, "a.txt", "one\n");
    fs::write(dir.join(".git/index"), b"DIRC garbage").unwrap();
    let found = info(&dir, true, Duration::from_secs(2));
    assert_eq!(found.head, Head::Branch("main".into()));
    assert_eq!(found.changes, None);
    fs::remove_dir_all(dir).unwrap();
}

fn stage(head: &str) -> GitInfo {
    GitInfo {
        head: Head::Branch(head.into()),
        upstream: None,
        changes: None,
    }
}

#[test]
fn a_reader_that_panics_fails_with_what_it_had() {
    let (sender, stages) = mpsc::channel();
    let reader = thread::spawn(move || {
        sender.send(stage("main")).unwrap();
        panic!("corrupt repository");
    });
    assert!(reader.join().is_err());
    assert_eq!(
        collect(&stages, 3, Duration::from_secs(2)),
        Read::Failed(Some(stage("main")))
    );
    let (sender, stages) = mpsc::channel::<GitInfo>();
    drop(sender);
    assert_eq!(
        collect(&stages, 2, Duration::from_secs(2)),
        Read::Failed(None)
    );
}

#[test]
fn a_reader_past_the_deadline_is_slow_and_a_complete_one_is_done() {
    let (sender, stages) = mpsc::channel();
    sender.send(stage("a")).unwrap();
    assert_eq!(
        collect(&stages, 2, Duration::from_millis(10)),
        Read::Slow(Some(stage("a")))
    );
    sender.send(stage("b")).unwrap();
    sender.send(stage("c")).unwrap();
    assert_eq!(
        collect(&stages, 2, Duration::from_secs(2)),
        Read::Done(stage("c"))
    );
}

#[test]
fn an_untracked_directory_counts_as_one_change() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("collapsed");
    commit(&dir, "a.txt", "one\n");
    fs::create_dir_all(dir.join("build/deep")).unwrap();
    for file in ["build/x", "build/y", "build/deep/z"] {
        fs::write(dir.join(file), "x\n").unwrap();
    }
    assert_eq!(
        info(&dir, true, Duration::from_secs(2)).changes,
        Some(Changes {
            files: 1,
            added: 0,
            removed: 0
        })
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_large_committed_file_counts_without_lines() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("large");
    commit(&dir, "big.txt", &"line\n".repeat(300_000));
    fs::write(dir.join("big.txt"), "small\n").unwrap();
    assert_eq!(
        info(&dir, true, Duration::from_secs(2)).changes,
        Some(Changes {
            files: 1,
            added: 0,
            removed: 0
        })
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn repository_filter_drivers_never_run() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("filter");
    let marker = dir.join("ran");
    commit(&dir, "a.txt", "one\n");
    commit(&dir, ".gitattributes", "*.txt filter=evil\n");
    run(
        &dir,
        &[
            "config",
            "filter.evil.clean",
            &format!("touch {}; cat", marker.display()),
        ],
    );
    run(
        &dir,
        &[
            "config",
            "filter.evil.process",
            &format!("touch {}", marker.display()),
        ],
    );
    fs::write(dir.join("a.txt"), "two\n").unwrap();
    let found = info(&dir, true, Duration::from_secs(2));
    assert_eq!(found.head, Head::Branch("main".into()));
    assert!(found.changes.is_some());
    assert!(!marker.exists(), "a filter driver ran");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn branch_names_are_sanitised_for_the_chrome() {
    if !git_available() {
        eprintln!("skipping git test: git is not on PATH");
        return;
    }
    let dir = temp("names");
    commit(&dir, "a.txt", "one\n");
    let long = "b".repeat(80);
    run(&dir, &["checkout", "-q", "-b", &long]);
    assert_eq!(
        info(&dir, false, Duration::from_secs(2)).head,
        Head::Branch("b".repeat(64))
    );
    run(&dir, &["checkout", "-q", "-b", "evil\u{202E}txt"]);
    let short = run(&dir, &["rev-parse", "--short=7", "HEAD"]);
    assert_eq!(
        info(&dir, false, Duration::from_secs(2)).head,
        Head::Detached(short)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_reader_discovers_the_repository_from_any_directory_inside_it() {
    let dir = temp("discover");
    let plain = dir.join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert_eq!(
        read(plain, false, Duration::from_secs(2)).0,
        Read::Failed(None)
    );
    if git_available() {
        let repo = dir.join("repo");
        fs::create_dir_all(repo.join("sub")).unwrap();
        commit(&repo, "a.txt", "one\n");
        let (outcome, straggler) = read(repo.join("sub"), false, Duration::from_secs(2));
        assert!(straggler.is_none());
        match outcome {
            Read::Done(found) => assert_eq!(found.head, Head::Branch("main".into())),
            other @ (Read::Slow(_) | Read::Failed(_)) => panic!("unfinished read: {other:?}"),
        }
    } else {
        eprintln!("skipping the repository half: git is not on PATH");
    }
    fs::remove_dir_all(dir).unwrap();
}
