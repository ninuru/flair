use super::*;
use crate::git::tests::{commit, git_available, temp};
use flair_core::isles::Head;
use std::{
    fs,
    os::unix::fs::symlink,
    process::{Child, Command},
    sync::mpsc,
};

fn job(cwd: &str, changes: bool, shells: &[u32]) -> Job {
    Job::Git {
        cwd: PathBuf::from(cwd),
        changes,
        shells: shells.to_vec(),
    }
}

struct Shell(Child);

impl Shell {
    fn in_dir(dir: &Path) -> Shell {
        Shell(
            Command::new("sleep")
                .arg("30")
                .current_dir(dir)
                .spawn()
                .unwrap(),
        )
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_batch_keeps_each_directory_once_and_merges_the_flags_and_shells() {
    let (cwds, contexts) = batch(vec![
        job("/a", false, &[1]),
        job("/b", false, &[2]),
        job("/a", true, &[3]),
        Job::Contexts,
        job("/b", false, &[2]),
    ]);
    let expected = BTreeMap::from([
        (PathBuf::from("/a"), (true, BTreeSet::from([1, 3]))),
        (PathBuf::from("/b"), (false, BTreeSet::from([2]))),
    ]);
    assert_eq!(cwds, expected);
    assert!(contexts);
    let (cwds, contexts) = batch(vec![job("/c", true, &[])]);
    assert_eq!(cwds.len(), 1);
    assert!(!contexts);
}

#[test]
fn a_directory_counts_only_when_a_shell_really_is_there() {
    let dir = temp("inside");
    let here = dir.join("here");
    let elsewhere = dir.join("elsewhere");
    fs::create_dir_all(&here).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    symlink(&here, dir.join("link")).unwrap();
    let shell = Shell::in_dir(&here);
    let other = Shell::in_dir(&elsewhere);
    assert!(inside(&here, &BTreeSet::from([shell.pid()])));
    assert!(inside(&dir.join("link"), &BTreeSet::from([shell.pid()])));
    assert!(inside(&here, &BTreeSet::from([other.pid(), shell.pid()])));
    assert!(!inside(&elsewhere, &BTreeSet::from([shell.pid()])));
    assert!(!inside(&here, &BTreeSet::new()));
    assert!(!inside(
        &dir.join("missing"),
        &BTreeSet::from([shell.pid()])
    ));
    assert!(!inside(&here, &BTreeSet::from([u32::MAX])));
    drop((shell, other));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_docker_context_is_the_current_one_unless_it_is_the_default() {
    let config = r#"{"auths": {"x": {}}, "currentContext": "colima"}"#;
    assert_eq!(docker_context(config), Some("colima".into()));
    let spaced = "{\n  \"currentContext\"  :\n   \"desktop-linux\"\n}";
    assert_eq!(docker_context(spaced), Some("desktop-linux".into()));
    assert_eq!(docker_context(r#"{"currentContext": "default"}"#), None);
    assert_eq!(docker_context(r#"{"auths": {}}"#), None);
    assert_eq!(docker_context(r#"{"currentContext": ""}"#), None);
    assert_eq!(docker_context(r#"{"currentContext": "a\"b"}"#), None);
    assert_eq!(docker_context(r#"{"currentContext": 7}"#), None);
}

#[test]
fn the_kube_context_is_the_top_level_current_context() {
    let config = "apiVersion: v1\ncurrent-context: prod\nkind: Config\n";
    assert_eq!(kube_context(config), Some("prod".into()));
    assert_eq!(
        kube_context("current-context: \"dev\"\n"),
        Some("dev".into())
    );
    assert_eq!(kube_context("current-context: 'dev'"), Some("dev".into()));
    let nested = "contexts:\n  current-context: nested\n";
    assert_eq!(kube_context(nested), None);
    assert_eq!(kube_context("current-context:\n"), None);
    assert_eq!(kube_context("current-context: \"\"\n"), None);
    assert_eq!(kube_context("kind: Config\n"), None);
}

#[test]
fn contexts_with_invisible_characters_are_dropped_and_long_ones_capped() {
    let bidi = "{\"currentContext\": \"pr\u{202E}od\"}";
    assert_eq!(docker_context(bidi), None);
    assert_eq!(kube_context("current-context: a\u{1b}b\n"), None);
    assert_eq!(kube_context("current-context: a\u{200B}b\n"), None);
    let long = "k".repeat(100);
    assert_eq!(
        kube_context(&format!("current-context: {long}\n")),
        Some("k".repeat(64))
    );
    let docker = format!("{{\"currentContext\": \"{long}\"}}");
    assert_eq!(docker_context(&docker), Some("k".repeat(64)));
}

#[test]
fn contexts_come_from_the_given_files() {
    let dir = temp("contexts");
    let docker = dir.join("config.json");
    let kube = dir.join("kube");
    fs::write(&docker, r#"{"currentContext": "colima"}"#).unwrap();
    fs::write(&kube, "current-context: prod\n").unwrap();
    let contexts = read_contexts(None, &docker, &kube);
    assert_eq!(contexts.docker, Some("colima".into()));
    assert_eq!(contexts.kube, Some("prod".into()));
    let missing = read_contexts(None, &dir.join("none.json"), &dir.join("none"));
    assert_eq!(missing, Contexts::default());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_docker_context_variable_wins_over_the_config_file() {
    let dir = temp("docker-env");
    let docker = dir.join("config.json");
    let kube = dir.join("kube");
    fs::write(&docker, r#"{"currentContext": "colima"}"#).unwrap();
    assert_eq!(
        read_contexts(Some("remote"), &docker, &kube).docker,
        Some("remote".into())
    );
    assert_eq!(read_contexts(Some("default"), &docker, &kube).docker, None);
    assert_eq!(
        read_contexts(Some(""), &docker, &kube).docker,
        Some("colima".into())
    );
    assert_eq!(read_contexts(Some("a\u{1b}b"), &docker, &kube).docker, None);
    fs::remove_dir_all(dir).unwrap();
}

fn reads(events: Vec<Event>) -> Vec<(PathBuf, Option<Head>)> {
    let mut reads: Vec<_> = events
        .into_iter()
        .map(|event| match event {
            Event::GitRead(cwd, info) => (cwd, info.map(|info| info.head)),
            Event::ClientConnected(_)
            | Event::Client(_, _)
            | Event::ClientGone(_)
            | Event::PtyOutput(_, _)
            | Event::PtyExited(_)
            | Event::PtySpawnFailed(_)
            | Event::Timer(_)
            | Event::ConfigRead(_)
            | Event::ContextsRead(_) => panic!("unexpected event"),
        })
        .collect();
    reads.sort_by(|a, b| a.0.cmp(&b.0));
    reads
}

#[test]
fn the_worker_answers_every_directory_and_stops_with_its_queue() {
    if !git_available() {
        eprintln!("skipping git worker test: git is not on PATH");
        return;
    }
    let dir = temp("worker");
    let repo = dir.join("repo");
    let plain = dir.join("plain");
    let planted = dir.join("planted");
    fs::create_dir_all(repo.join("sub")).unwrap();
    fs::create_dir_all(&plain).unwrap();
    commit(&repo, "a.txt", "one\n");
    commit(&planted, "b.txt", "two\n");
    let shells = [
        Shell::in_dir(&repo.join("sub")),
        Shell::in_dir(&repo),
        Shell::in_dir(&plain),
    ];
    let (jobs, queue) = mpsc::channel();
    for (cwd, shell) in [repo.join("sub"), repo.clone(), plain.clone()]
        .into_iter()
        .zip(&shells)
    {
        jobs.send(Job::Git {
            cwd,
            changes: true,
            shells: vec![shell.pid()],
        })
        .unwrap();
    }
    jobs.send(Job::Git {
        cwd: planted.clone(),
        changes: true,
        shells: vec![shells[2].pid()],
    })
    .unwrap();
    drop(jobs);
    let mut events = Vec::new();
    let nowhere = PathBuf::from("/nonexistent");
    work(queue, None, nowhere.clone(), nowhere, |event| {
        events.push(event);
        true
    });
    let main = Some(Head::Branch("main".into()));
    let mut expected = vec![
        (repo.join("sub"), main.clone()),
        (repo, main),
        (plain, None),
        (planted, None),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(reads(events), expected);
    drop(shells);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_nested_shell_in_the_foreground_vouches_for_its_directory() {
    if !git_available() {
        eprintln!("skipping git worker test: git is not on PATH");
        return;
    }
    let dir = temp("nested");
    let repo = dir.join("repo");
    let outer = dir.join("outer");
    let planted = dir.join("planted");
    fs::create_dir_all(&outer).unwrap();
    commit(&repo, "a.txt", "one\n");
    commit(&planted, "b.txt", "two\n");
    let pane = Shell::in_dir(&outer);
    let nested = Shell(
        Command::new("sh")
            .arg("-c")
            .arg("cd \"$1\" && exec sleep 30")
            .arg("sh")
            .arg(&repo)
            .current_dir(&outer)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !inside(&repo, &BTreeSet::from([nested.pid()])) {
        assert!(
            Instant::now() < deadline,
            "the nested shell never reached the repo"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!inside(&repo, &BTreeSet::from([pane.pid()])));
    let (jobs, queue) = mpsc::channel();
    for cwd in [repo.clone(), planted.clone()] {
        jobs.send(Job::Git {
            cwd,
            changes: false,
            shells: vec![pane.pid(), nested.pid()],
        })
        .unwrap();
    }
    drop(jobs);
    let mut events = Vec::new();
    let nowhere = PathBuf::from("/nonexistent");
    work(queue, None, nowhere.clone(), nowhere, |event| {
        events.push(event);
        true
    });
    let mut expected = vec![(repo, Some(Head::Branch("main".into()))), (planted, None)];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(reads(events), expected);
    drop((pane, nested));
    fs::remove_dir_all(dir).unwrap();
}

fn branch(name: &str) -> GitInfo {
    GitInfo {
        head: Head::Branch(name.into()),
        upstream: None,
        changes: None,
    }
}

fn blocked() -> (mpsc::Sender<()>, JoinHandle<()>) {
    let (release, wait) = mpsc::channel::<()>();
    let reader = std::thread::spawn(move || {
        let _ = wait.recv();
    });
    (release, reader)
}

fn untouched(_: PathBuf, _: bool) -> (Read, Option<JoinHandle<()>>) {
    panic!("the repository was read while a straggler lives")
}

#[test]
fn while_a_straggler_lives_every_directory_gets_its_last_facts_without_a_read() {
    let mut reads = Reads::default();
    let cached = PathBuf::from("/repo");
    let answer = reads.git(cached.clone(), true, |_, _| {
        (Read::Done(branch("main")), None)
    });
    assert_eq!(answer, Some(branch("main")));
    let (release, reader) = blocked();
    let slow = reads.git(PathBuf::from("/huge"), true, move |_, _| {
        (Read::Slow(Some(branch("big"))), Some(reader))
    });
    assert_eq!(slow, Some(branch("big")));
    assert_eq!(
        reads.git(cached.clone(), true, untouched),
        Some(branch("main"))
    );
    assert_eq!(
        reads.git(PathBuf::from("/huge"), true, untouched),
        Some(branch("big"))
    );
    assert_eq!(reads.git(PathBuf::from("/new"), true, untouched), None);
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while reads
        .straggler
        .as_ref()
        .is_some_and(|reader| !reader.is_finished())
    {
        assert!(Instant::now() < deadline, "the straggler never finished");
        std::thread::sleep(Duration::from_millis(5));
    }
    let fresh = reads.git(cached, true, |_, _| (Read::Done(branch("next")), None));
    assert_eq!(fresh, Some(branch("next")));
}

#[test]
fn a_slow_directory_is_read_without_changes_and_a_failed_one_forgets_its_facts() {
    let mut reads = Reads::default();
    let dir = PathBuf::from("/repo");
    reads.git(dir.clone(), true, |cwd, changes| {
        assert_eq!(cwd, PathBuf::from("/repo"));
        assert!(changes);
        (Read::Slow(Some(branch("main"))), None)
    });
    reads.git(dir.clone(), true, |_, changes| {
        assert!(!changes);
        (Read::Failed(None), None)
    });
    let (release, reader) = blocked();
    reads.git(PathBuf::from("/other"), false, move |_, _| {
        (Read::Slow(None), Some(reader))
    });
    assert_eq!(reads.git(dir, true, untouched), None);
    release.send(()).unwrap();
}
