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

fn entries(dir: &Path) -> Vec<std::ffi::OsString> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

#[test]
fn saving_a_key_returns_the_text_and_the_config() {
    let dir = private_dir("save-key");
    let path = dir.join("flair.toml");
    let text = "# my flair\n[font]\n# points\nsize = 13.0 # big\nfamily = \"x\"\n";
    fs::write(&path, text).unwrap();
    let (written, config) = save(&path, "font.size", Some(&Value::Float(14.5))).unwrap();
    let expected = "# my flair\n[font]\n# points\nsize = 14.5 # big\nfamily = \"x\"\n";
    assert_eq!(written, expected);
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    assert_eq!(config.font.size, 14.5);
    assert_eq!(config.font.family, "x");
    assert_eq!(entries(&dir), ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn resetting_a_key_removes_it() {
    let dir = private_dir("save-reset");
    let path = dir.join("flair.toml");
    fs::write(&path, "theme = \"gruvbox-dark\"\n[font]\nsize = 15.0\n").unwrap();
    let (written, config) = save(&path, "font.size", None).unwrap();
    assert_eq!(written, "theme = \"gruvbox-dark\"\n[font]\n");
    assert_eq!(fs::read_to_string(&path).unwrap(), written);
    assert_eq!(config, Config::default().with_theme("gruvbox-dark"));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_invalid_value_leaves_the_file_alone() {
    let dir = private_dir("save-invalid");
    let path = dir.join("flair.toml");
    let text = "[font]\nsize = 13.0\n";
    fs::write(&path, text).unwrap();
    let error = save(&path, "font.size", Some(&Value::Float(0.0))).unwrap_err();
    assert!(error.to_string().contains("font.size"), "{error}");
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    let error = save(&path, "font", Some(&Value::Integer(3))).unwrap_err();
    assert!(!error.to_string().is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    assert_eq!(entries(&dir), ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_key_keeps_mode_and_symlink() {
    let dir = private_dir("save-key-link");
    let target = dir.join("dotfiles").join("flair.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "[window]\npadding_x = 3\n").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.join("flair.toml");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let (_, config) = save(&link, "window.padding_x", Some(&Value::Integer(9))).unwrap();
    assert_eq!(config.window.padding_x, 9);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "[window]\npadding_x = 9\n"
    );
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(entries(target.parent().unwrap()), ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_stale_temporary_file_does_not_block_a_save() {
    let dir = private_dir("save-stale-tmp");
    let path = dir.join("flair.toml");
    fs::write(&path, "").unwrap();
    let stale = dir.join(format!("flair.toml.{}.tmp", process::id()));
    fs::write(&stale, "left over").unwrap();
    let (text, _) = save(
        &path,
        "theme",
        Some(&Value::Text("gruvbox-dark".to_owned())),
    )
    .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    assert_eq!(entries(&dir), ["flair.toml"]);
    fs::remove_dir_all(&dir).unwrap();
}
