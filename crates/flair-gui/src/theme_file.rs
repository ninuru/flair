use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process,
};

use flair_core::{config::Config, config_edit::set_theme, theme::Theme};

use crate::notice::Notice;

pub(crate) fn save_theme(path: &Path, name: &str) -> io::Result<Config> {
    let path = match fs::canonicalize(path) {
        Ok(real) => real,
        Err(error) if error.kind() == io::ErrorKind::NotFound => path.to_owned(),
        Err(error) => return Err(error),
    };
    let (text, permissions) = match fs::read_to_string(&path) {
        Ok(text) => (text, Some(fs::metadata(&path)?.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (String::new(), None),
        Err(error) => return Err(error),
    };
    let text = set_theme(&text, name);
    let config = Config::from_toml(&text).map_err(io::Error::other)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", process::id()));
    let tmp = PathBuf::from(tmp);
    let replaced = replace_file(&tmp, &path, &text, permissions);
    if replaced.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    replaced.map(|()| config)
}

fn replace_file(
    tmp: &Path,
    path: &Path,
    text: &str,
    permissions: Option<fs::Permissions>,
) -> io::Result<()> {
    let mode = permissions.as_ref().map_or(0o666, |p| p.mode() & 0o7777);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(tmp)?;
    file.write_all(text.as_bytes())?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    drop(file);
    fs::rename(tmp, path)
}

pub(crate) fn resolve_theme(config: &Config, path: &Path) -> (Theme, Option<Notice>) {
    let (theme, error) = config.load_theme(&Config::themes_dir(path));
    let notice = error.map(|error| {
        log::warn!("{error}; using defaults");
        Notice::theme_fallback(&error)
    });
    (theme, notice)
}

#[cfg(test)]
#[path = "theme_file_tests.rs"]
mod tests;
