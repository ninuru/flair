use crate::{
    event::Event,
    git::{self, Read},
};
use flair_core::isles::{GitInfo, label};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, iter,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    thread::JoinHandle,
    time::{Duration, Instant},
};

const STATUS_DEADLINE: Duration = Duration::from_secs(2);
const SLOW_PAUSE: Duration = Duration::from_secs(60);
const KNOWN_CAP: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contexts {
    pub docker: Option<String>,
    pub kube: Option<String>,
}

#[derive(Debug)]
pub(crate) enum Job {
    Git {
        cwd: PathBuf,
        changes: bool,
        shells: Vec<u32>,
    },
    Contexts,
}

type Wanted = BTreeMap<PathBuf, (bool, BTreeSet<u32>)>;

pub(crate) fn batch(jobs: impl IntoIterator<Item = Job>) -> (Wanted, bool) {
    let mut cwds: Wanted = BTreeMap::new();
    let mut contexts = false;
    for job in jobs {
        match job {
            Job::Git {
                cwd,
                changes,
                shells,
            } => {
                let entry = cwds.entry(cwd).or_default();
                entry.0 |= changes;
                entry.1.extend(shells);
            }
            Job::Contexts => contexts = true,
        }
    }
    (cwds, contexts)
}

pub(crate) fn inside(cwd: &Path, shells: &BTreeSet<u32>) -> bool {
    let Ok(claimed) = fs::canonicalize(cwd) else {
        return false;
    };
    shells
        .iter()
        .any(|pid| fs::canonicalize(format!("/proc/{pid}/cwd")).is_ok_and(|real| real == claimed))
}

#[derive(Debug, Default)]
pub(crate) struct Reads {
    slow: BTreeMap<PathBuf, Instant>,
    straggler: Option<JoinHandle<()>>,
    known: BTreeMap<PathBuf, GitInfo>,
}

impl Reads {
    pub(crate) fn git(
        &mut self,
        cwd: PathBuf,
        changes: bool,
        read: impl FnOnce(PathBuf, bool) -> (Read, Option<JoinHandle<()>>),
    ) -> Option<GitInfo> {
        if self
            .straggler
            .as_ref()
            .is_some_and(|reader| !reader.is_finished())
        {
            return self.known.get(&cwd).cloned();
        }
        self.slow.retain(|_, since| since.elapsed() < SLOW_PAUSE);
        let changes = changes && !self.slow.contains_key(&cwd);
        let (outcome, left) = read(cwd.clone(), changes);
        self.straggler = left;
        let info = match outcome {
            Read::Done(info) => Some(info),
            Read::Slow(info) => {
                log::debug!("git facts for {} are slow, skipping changes", cwd.display());
                self.slow.insert(cwd.clone(), Instant::now());
                info
            }
            Read::Failed(info) => {
                if info.is_some() {
                    log::warn!("reading git facts for {} failed", cwd.display());
                    self.slow.insert(cwd.clone(), Instant::now());
                }
                info
            }
        };
        match &info {
            Some(found) => {
                if self.known.len() >= KNOWN_CAP && !self.known.contains_key(&cwd) {
                    self.known.clear();
                }
                self.known.insert(cwd, found.clone());
            }
            None => {
                self.known.remove(&cwd);
            }
        }
        info
    }
}

pub(crate) fn work(
    jobs: Receiver<Job>,
    docker_context: Option<String>,
    docker: PathBuf,
    kube: PathBuf,
    mut send: impl FnMut(Event) -> bool,
) {
    let mut reads = Reads::default();
    while let Ok(first) = jobs.recv() {
        let (cwds, contexts) = batch(iter::once(first).chain(jobs.try_iter()));
        if contexts
            && !send(Event::ContextsRead(read_contexts(
                docker_context.as_deref(),
                &docker,
                &kube,
            )))
        {
            return;
        }
        for (cwd, (changes, shells)) in cwds {
            let info = if inside(&cwd, &shells) {
                reads.git(cwd.clone(), changes, |cwd, changes| {
                    git::read(cwd, changes, STATUS_DEADLINE)
                })
            } else {
                None
            };
            if !send(Event::GitRead(cwd, info)) {
                return;
            }
        }
    }
}

pub(crate) fn context_sources() -> (Option<String>, PathBuf, PathBuf) {
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let docker = env::var_os("DOCKER_CONFIG")
        .map_or_else(|| home.join(".docker"), PathBuf::from)
        .join("config.json");
    let kube = env::var_os("KUBECONFIG")
        .and_then(|paths| env::split_paths(&paths).find(|path| !path.as_os_str().is_empty()))
        .unwrap_or_else(|| home.join(".kube/config"));
    (env::var("DOCKER_CONTEXT").ok(), docker, kube)
}

pub(crate) fn read_contexts(docker_override: Option<&str>, docker: &Path, kube: &Path) -> Contexts {
    let docker = match docker_override.filter(|name| !name.is_empty()) {
        Some(name) => current(name),
        None => fs::read_to_string(docker)
            .ok()
            .and_then(|json| docker_context(&json)),
    };
    Contexts {
        docker,
        kube: fs::read_to_string(kube)
            .ok()
            .and_then(|yaml| kube_context(&yaml)),
    }
}

fn current(name: &str) -> Option<String> {
    (name != "default").then(|| label(name)).flatten()
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
    (!value.is_empty() && !value.contains('\\'))
        .then(|| current(value))
        .flatten()
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
    (!value.is_empty()).then(|| label(value)).flatten()
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
