use flair_core::isles::{AheadBehind, Changes, GitInfo, Head, label};
use gix::{
    ObjectId, Repository, Tree,
    bstr::{BStr, BString},
    diff::blob::{Algorithm, Diff, InternedInput, sources::byte_lines},
    dir::entry::Status,
    remote::Direction,
    status::{Item, UntrackedFiles, index_worktree},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const WALK_CAP: usize = 1000;
const LARGE_FILE: u64 = 1 << 20;
const BINARY_PROBE: usize = 8000;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Read {
    Done(GitInfo),
    Slow(Option<GitInfo>),
    Failed(Option<GitInfo>),
}

pub(crate) fn open(cwd: &Path) -> Option<(PathBuf, Repository)> {
    let mut repo = gix::discover(cwd).ok()?;
    let root = repo.workdir()?.to_path_buf();
    let mut config = repo.config_snapshot_mut();
    let filters: Vec<_> = config
        .sections_and_ids_by_name("filter")
        .into_iter()
        .flatten()
        .map(|(_, id)| id)
        .collect();
    for id in filters {
        config.remove_section_by_id(id);
    }
    config.commit().ok()?;
    Some((root, repo))
}

pub(crate) fn read(
    cwd: PathBuf,
    changes: bool,
    deadline: Duration,
) -> (Read, Option<JoinHandle<()>>) {
    let interrupt = Arc::new(AtomicBool::new(false));
    let (sender, stages) = mpsc::channel();
    let flag = interrupt.clone();
    let spawned = thread::Builder::new()
        .name("flair-git".into())
        .spawn(move || {
            let Some((_, repo)) = open(&cwd) else {
                return;
            };
            let mut info = GitInfo {
                head: head(&repo),
                upstream: None,
                changes: None,
            };
            if sender.send(info.clone()).is_err() {
                return;
            }
            info.upstream = upstream(&repo);
            if sender.send(info.clone()).is_err() || !changes {
                return;
            }
            info.changes = self::changes(&repo, &flag);
            let _ = sender.send(info);
        });
    let handle = match spawned {
        Ok(handle) => handle,
        Err(error) => {
            log::warn!("could not start a git reader: {error}");
            return (Read::Failed(None), None);
        }
    };
    let read = collect(&stages, if changes { 3 } else { 2 }, deadline);
    interrupt.store(true, Ordering::Release);
    match read {
        Read::Slow(_) => (read, Some(handle)),
        Read::Done(_) | Read::Failed(_) => {
            let _ = handle.join();
            (read, None)
        }
    }
}

fn collect(stages: &Receiver<GitInfo>, count: usize, deadline: Duration) -> Read {
    let end = Instant::now() + deadline;
    let mut last = None;
    for _ in 0..count {
        match stages.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(info) => last = Some(info),
            Err(RecvTimeoutError::Timeout) => return Read::Slow(last),
            Err(RecvTimeoutError::Disconnected) => return Read::Failed(last),
        }
    }
    last.map_or(Read::Failed(None), Read::Done)
}

fn head(repo: &Repository) -> Head {
    let detached = || {
        Head::Detached(
            repo.head_id()
                .map(|id| id.to_hex_with_len(7).to_string())
                .unwrap_or_default(),
        )
    };
    match repo.head_name() {
        Ok(Some(name)) => label(&name.shorten().to_string()).map_or_else(detached, Head::Branch),
        Ok(None) | Err(_) => detached(),
    }
}

fn upstream(repo: &Repository) -> Option<AheadBehind> {
    let local = repo.head_id().ok()?.detach();
    let name = repo
        .head_ref()
        .ok()??
        .remote_tracking_ref_name(Direction::Fetch)?
        .ok()?;
    let remote = repo
        .find_reference(name.as_ref())
        .ok()?
        .peel_to_id()
        .ok()?
        .detach();
    Some(AheadBehind {
        ahead: walk(repo, local, remote),
        behind: walk(repo, remote, local),
    })
}

fn walk(repo: &Repository, tip: ObjectId, hidden: ObjectId) -> u32 {
    repo.rev_walk([tip])
        .with_hidden([hidden])
        .all()
        .map_or(0, |commits| {
            u32::try_from(commits.take(WALK_CAP).map_while(Result::ok).count()).unwrap_or(u32::MAX)
        })
}

fn changes(repo: &Repository, interrupt: &Arc<AtomicBool>) -> Option<Changes> {
    let status = repo
        .status(gix::progress::Discard)
        .ok()?
        .untracked_files(UntrackedFiles::Collapsed)
        .index_worktree_rewrites(None)
        .should_interrupt_owned(interrupt.clone())
        .into_iter(None::<BString>)
        .ok()?;
    let mut files = BTreeSet::new();
    let mut tracked = BTreeSet::new();
    for item in status {
        let item = item.ok()?;
        let path = item.location().to_owned();
        match &item {
            Item::IndexWorktree(index_worktree::Item::DirectoryContents { entry, .. }) => {
                if entry.status != Status::Untracked {
                    continue;
                }
            }
            Item::IndexWorktree(
                index_worktree::Item::Modification { .. } | index_worktree::Item::Rewrite { .. },
            )
            | Item::TreeIndex(_) => {
                tracked.insert(path.clone());
            }
        }
        files.insert(path);
    }
    let tree = repo.head_tree().ok();
    let (mut added, mut removed) = (0u32, 0u32);
    for path in &tracked {
        if interrupt.load(Ordering::Acquire) {
            return None;
        }
        let (more, fewer) = lines(repo, tree.as_ref(), path.as_ref());
        added = added.saturating_add(more);
        removed = removed.saturating_add(fewer);
    }
    (!interrupt.load(Ordering::Acquire)).then(|| Changes {
        files: u32::try_from(files.len()).unwrap_or(u32::MAX),
        added,
        removed,
    })
}

fn lines(repo: &Repository, tree: Option<&Tree<'_>>, path: &BStr) -> (u32, u32) {
    let Some(after) = on_disk(repo, path) else {
        return (0, 0);
    };
    let Some(before) = committed(repo, tree, path) else {
        return (0, 0);
    };
    if !text(&before) || !text(&after) {
        return (0, 0);
    }
    let input = InternedInput::new(byte_lines(&before), byte_lines(&after));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    (diff.count_additions(), diff.count_removals())
}

fn committed(repo: &Repository, tree: Option<&Tree<'_>>, path: &BStr) -> Option<Vec<u8>> {
    let Some(entry) = tree.and_then(|tree| {
        tree.lookup_entry_by_path(gix::path::from_bstr(path).ok()?)
            .ok()
            .flatten()
    }) else {
        return Some(Vec::new());
    };
    if !entry.mode().is_blob() || repo.find_header(entry.oid()).ok()?.size() > LARGE_FILE {
        return None;
    }
    entry.object().ok().map(|object| object.detach().data)
}

fn on_disk(repo: &Repository, path: &BStr) -> Option<Vec<u8>> {
    let file = repo.workdir_path(path)?;
    match fs::symlink_metadata(&file) {
        Err(_) => Some(Vec::new()),
        Ok(meta) if meta.is_file() && meta.len() <= LARGE_FILE => fs::read(&file).ok(),
        Ok(_) => None,
    }
}

fn text(data: &[u8]) -> bool {
    data.len() as u64 <= LARGE_FILE && !data[..data.len().min(BINARY_PROBE)].contains(&0)
}

#[cfg(test)]
#[path = "git_tests.rs"]
pub(crate) mod tests;
