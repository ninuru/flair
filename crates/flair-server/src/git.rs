use flair_core::isles::{AheadBehind, Changes, GitInfo, Head};
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
        mpsc,
    },
    thread,
    time::Duration,
};

const WALK_CAP: usize = 1000;
const LARGE_FILE: u64 = 1 << 20;
const BINARY_PROBE: usize = 8000;

pub(crate) fn open(cwd: &Path) -> Option<(PathBuf, Repository)> {
    let repo = gix::discover(cwd).ok()?;
    let root = repo.workdir()?.to_path_buf();
    Some((root, repo))
}

pub(crate) fn read(repo: &Repository, changes: bool, deadline: Duration) -> GitInfo {
    GitInfo {
        head: head(repo),
        upstream: upstream(repo),
        changes: if changes {
            within(repo, deadline)
        } else {
            None
        },
    }
}

fn head(repo: &Repository) -> Head {
    match repo.head_name() {
        Ok(Some(name)) => Head::Branch(name.shorten().to_string()),
        Ok(None) | Err(_) => Head::Detached(
            repo.head_id()
                .map(|id| id.to_hex_with_len(7).to_string())
                .unwrap_or_default(),
        ),
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

fn within(repo: &Repository, deadline: Duration) -> Option<Changes> {
    let interrupt = Arc::new(AtomicBool::new(false));
    let (done, result) = mpsc::channel();
    thread::scope(|scope| {
        let root = repo.workdir()?.to_path_buf();
        let flag = interrupt.clone();
        let spawned = thread::Builder::new()
            .name("flair-git-status".into())
            .spawn_scoped(scope, move || {
                let found = gix::discover(&root)
                    .ok()
                    .and_then(|repo| changes(&repo, &flag));
                let _ = done.send(found);
            });
        if spawned.is_err() {
            return None;
        }
        let changes = result.recv_timeout(deadline).ok().flatten();
        interrupt.store(true, Ordering::Release);
        changes
    })
}

fn changes(repo: &Repository, interrupt: &Arc<AtomicBool>) -> Option<Changes> {
    let status = repo
        .status(gix::progress::Discard)
        .ok()?
        .untracked_files(UntrackedFiles::Files)
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
    let (Some(before), Some(after)) = (committed(tree, path), on_disk(repo, path)) else {
        return (0, 0);
    };
    if !text(&before) || !text(&after) {
        return (0, 0);
    }
    let input = InternedInput::new(byte_lines(&before), byte_lines(&after));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    (diff.count_additions(), diff.count_removals())
}

fn committed(tree: Option<&Tree<'_>>, path: &BStr) -> Option<Vec<u8>> {
    let Some(entry) = tree.and_then(|tree| {
        tree.lookup_entry_by_path(gix::path::from_bstr(path).ok()?)
            .ok()
            .flatten()
    }) else {
        return Some(Vec::new());
    };
    if !entry.mode().is_blob() {
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
