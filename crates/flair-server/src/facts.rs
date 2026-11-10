use crate::{event::Event, git};
use gix::Repository;
use std::{
    collections::BTreeMap,
    env, fs, iter,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

const STATUS_DEADLINE: Duration = Duration::from_secs(2);
const SLOW_PAUSE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contexts {
    pub docker: Option<String>,
    pub kube: Option<String>,
}

#[derive(Debug)]
pub(crate) enum Job {
    Git { cwd: PathBuf, changes: bool },
    Contexts,
}

pub(crate) fn batch(jobs: impl IntoIterator<Item = Job>) -> (BTreeMap<PathBuf, bool>, bool) {
    let mut cwds = BTreeMap::new();
    let mut contexts = false;
    for job in jobs {
        match job {
            Job::Git { cwd, changes } => *cwds.entry(cwd).or_insert(false) |= changes,
            Job::Contexts => contexts = true,
        }
    }
    (cwds, contexts)
}

pub(crate) fn work(
    jobs: Receiver<Job>,
    docker: PathBuf,
    kube: PathBuf,
    mut send: impl FnMut(Event) -> bool,
) {
    let mut slow: BTreeMap<PathBuf, Instant> = BTreeMap::new();
    while let Ok(first) = jobs.recv() {
        let (cwds, contexts) = batch(iter::once(first).chain(jobs.try_iter()));
        if contexts && !send(Event::ContextsRead(read_contexts(&docker, &kube))) {
            return;
        }
        slow.retain(|_, since| since.elapsed() < SLOW_PAUSE);
        let mut roots: BTreeMap<PathBuf, (Repository, bool, Vec<PathBuf>)> = BTreeMap::new();
        for (cwd, changes) in cwds {
            match git::open(&cwd) {
                Some((root, repo)) => {
                    let group = roots
                        .entry(root)
                        .or_insert_with(|| (repo, false, Vec::new()));
                    group.1 |= changes;
                    group.2.push(cwd);
                }
                None => {
                    if !send(Event::GitRead(cwd, None)) {
                        return;
                    }
                }
            }
        }
        for (root, (repo, changes, cwds)) in roots {
            let changes = changes && !slow.contains_key(&root);
            let info = git::read(&repo, changes, STATUS_DEADLINE);
            if changes && info.changes.is_none() {
                log::debug!(
                    "git status for {} is slow, skipping changes",
                    root.display()
                );
                slow.insert(root, Instant::now());
            }
            for cwd in cwds {
                if !send(Event::GitRead(cwd, Some(info.clone()))) {
                    return;
                }
            }
        }
    }
}

pub(crate) fn context_files() -> (PathBuf, PathBuf) {
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let docker = env::var_os("DOCKER_CONFIG")
        .map_or_else(|| home.join(".docker"), PathBuf::from)
        .join("config.json");
    let kube = env::var_os("KUBECONFIG")
        .and_then(|paths| env::split_paths(&paths).find(|path| !path.as_os_str().is_empty()))
        .unwrap_or_else(|| home.join(".kube/config"));
    (docker, kube)
}

pub(crate) fn read_contexts(docker: &Path, kube: &Path) -> Contexts {
    Contexts {
        docker: fs::read_to_string(docker)
            .ok()
            .and_then(|json| docker_context(&json)),
        kube: fs::read_to_string(kube)
            .ok()
            .and_then(|yaml| kube_context(&yaml)),
    }
}

pub(crate) fn docker_context(json: &str) -> Option<String> {
    let key = "\"currentContext\"";
    let rest = &json[json.find(key)? + key.len()..];
    let rest = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let value = &rest[..rest.find('"')?];
    (!value.is_empty() && value != "default" && !value.contains('\\')).then(|| value.to_owned())
}

pub(crate) fn kube_context(yaml: &str) -> Option<String> {
    let value = yaml
        .lines()
        .find_map(|line| line.strip_prefix("current-context:"))?
        .trim();
    let value = ['"', '\'']
        .into_iter()
        .find_map(|quote| value.strip_prefix(quote)?.strip_suffix(quote))
        .unwrap_or(value);
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
