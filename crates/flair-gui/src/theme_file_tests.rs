use super::*;
use std::{env, os::unix::fs::PermissionsExt, path::PathBuf, process};

fn private_dir(tag: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("flair-{tag}-{}", process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

#[test]
fn saving_a_theme_keeps_comments_and_adds_the_theme_line() {
    let dir = private_dir("save-theme");
    let path = dir.join("flair.toml");
    let text = "# my flair\n\n[font]\n# size in points\nsize = 13.0 # big\n";
    fs::write(&path, text).unwrap();
    let config = save_theme(&path, "gruvbox-dark").unwrap();
    assert_eq!(config.theme, "gruvbox-dark");
    assert_eq!(config.font.size, 13.0);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("theme = \"gruvbox-dark\"\n{text}")
    );
    let replaced = save_theme(&path, "catppuccin-mocha").unwrap();
    assert_eq!(replaced.theme, "catppuccin-mocha");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("theme = \"catppuccin-mocha\"\n{text}")
    );
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_theme_creates_a_missing_config() {
    let dir = private_dir("save-theme-missing");
    let path = dir.join("flair").join("flair.toml");
    let config = save_theme(&path, "gruvbox-dark").unwrap();
    assert_eq!(config, Config::default().with_theme("gruvbox-dark"));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "theme = \"gruvbox-dark\"\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_theme_keeps_a_config_that_does_not_parse() {
    let dir = private_dir("save-theme-broken");
    let path = dir.join("flair.toml");
    let text = "[font]\nsize = \"big\"\n";
    fs::write(&path, text).unwrap();
    assert!(save_theme(&path, "gruvbox-dark").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_theme_writes_through_a_symlinked_config() {
    let dir = private_dir("save-theme-link");
    let target = dir.join("dotfiles").join("flair.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "[font]\nsize = 13.0\n").unwrap();
    let link = dir.join("flair.toml");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let config = save_theme(&link, "gruvbox-dark").unwrap();
    assert_eq!(config.theme, "gruvbox-dark");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "theme = \"gruvbox-dark\"\n[font]\nsize = 13.0\n"
    );
    let entries: Vec<_> = fs::read_dir(target.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries, ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_theme_keeps_the_config_permissions() {
    let dir = private_dir("save-theme-mode");
    let path = dir.join("flair.toml");
    fs::write(&path, "").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    save_theme(&path, "gruvbox-dark").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_dir_all(&dir).unwrap();
}
