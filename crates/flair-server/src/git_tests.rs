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

fn info(dir: &Path, changes: bool, deadline: Duration) -> GitInfo {
    let (_, repo) = open(dir).unwrap();
    read(&repo, changes, deadline)
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
    let found = info(&dir, true, Duration::ZERO);
    assert_eq!(found.head, Head::Branch("main".into()));
    assert_eq!(found.changes, None);
    fs::remove_dir_all(dir).unwrap();
}
