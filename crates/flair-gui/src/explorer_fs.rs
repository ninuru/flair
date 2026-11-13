use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
};

use crate::explorer::{Entry, Listing};

pub const LISTING_CAP: usize = 1000;
pub const SCAN_CAP: usize = 50_000;

pub fn read_listing(dir: &Path) -> Listing {
    let reader = match fs::read_dir(dir) {
        Ok(reader) => reader,
        Err(error) => {
            return Listing {
                dir: dir.to_path_buf(),
                entries: Vec::new(),
                more: 0,
                error: Some(error.to_string()),
            };
        }
    };
    let mut entries: Vec<Entry> = reader
        .take(SCAN_CAP)
        .filter_map(Result::ok)
        .map(|item| {
            let dir = match item.file_type() {
                Ok(kind) if kind.is_symlink() => {
                    fs::metadata(item.path()).is_ok_and(|meta| meta.is_dir())
                }
                Ok(kind) => kind.is_dir(),
                Err(_) => false,
            };
            Entry {
                name: item.file_name(),
                dir,
            }
        })
        .collect();
    entries.sort_by_cached_key(|item| {
        (
            !item.dir,
            item.name.to_string_lossy().to_lowercase(),
            item.name.clone(),
        )
    });
    let more = entries.len().saturating_sub(LISTING_CAP);
    entries.truncate(LISTING_CAP);
    Listing {
        dir: dir.to_path_buf(),
        entries,
        more,
        error: None,
    }
}

pub struct FsWorker {
    tx: Sender<PathBuf>,
}

impl FsWorker {
    pub fn spawn(done: impl Fn(Listing) + Send + 'static) -> io::Result<FsWorker> {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        thread::Builder::new()
            .name("flair-explorer".into())
            .spawn(move || {
                for dir in rx {
                    done(read_listing(&dir));
                }
            })?;
        Ok(FsWorker { tx })
    }

    pub fn request(&self, dir: PathBuf) {
        if let Err(error) = self.tx.send(dir) {
            log::warn!("explorer worker is gone: {}", error.0.display());
        }
    }
}

#[cfg(test)]
#[path = "explorer_fs_tests.rs"]
mod tests;
