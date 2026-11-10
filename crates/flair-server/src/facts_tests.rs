use super::*;
use crate::git::tests::{commit, git_available, temp};
use flair_core::isles::Head;
use std::{fs, sync::mpsc};

fn job(cwd: &str, changes: bool) -> Job {
    Job::Git {
        cwd: PathBuf::from(cwd),
        changes,
    }
}

#[test]
fn a_batch_keeps_each_directory_once_and_merges_the_flags() {
    let (cwds, contexts) = batch(vec![
        job("/a", false),
        job("/b", false),
        job("/a", true),
        Job::Contexts,
        job("/b", false),
    ]);
    let expected = BTreeMap::from([(PathBuf::from("/a"), true), (PathBuf::from("/b"), false)]);
    assert_eq!(cwds, expected);
    assert!(contexts);
    let (cwds, contexts) = batch(vec![job("/c", true)]);
    assert_eq!(cwds.len(), 1);
    assert!(!contexts);
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
fn contexts_come_from_the_given_files() {
    let dir = temp("contexts");
    let docker = dir.join("config.json");
    let kube = dir.join("kube");
    fs::write(&docker, r#"{"currentContext": "colima"}"#).unwrap();
    fs::write(&kube, "current-context: prod\n").unwrap();
    let contexts = read_contexts(&docker, &kube);
    assert_eq!(contexts.docker, Some("colima".into()));
    assert_eq!(contexts.kube, Some("prod".into()));
    let missing = read_contexts(&dir.join("none.json"), &dir.join("none"));
    assert_eq!(missing, Contexts::default());
    fs::remove_dir_all(dir).unwrap();
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
    fs::create_dir_all(repo.join("sub")).unwrap();
    fs::create_dir_all(&plain).unwrap();
    commit(&repo, "a.txt", "one\n");
    let (jobs, queue) = mpsc::channel();
    for cwd in [repo.join("sub"), repo.clone(), plain.clone()] {
        jobs.send(Job::Git { cwd, changes: true }).unwrap();
    }
    drop(jobs);
    let mut events = Vec::new();
    let nowhere = PathBuf::from("/nonexistent");
    work(queue, nowhere.clone(), nowhere, |event| {
        events.push(event);
        true
    });
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
    let main = Some(Head::Branch("main".into()));
    let mut expected = vec![
        (repo.join("sub"), main.clone()),
        (repo, main),
        (plain, None),
    ];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(reads, expected);
    fs::remove_dir_all(dir).unwrap();
}
