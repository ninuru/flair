use super::*;
use std::{env, os::unix::fs::PermissionsExt, path::PathBuf, process};

#[test]
fn bar_press_goes_to_the_bar_and_other_bar_events_drop() {
    assert_eq!(
        route_mouse(true, MouseButton::Left, MouseAction::Press, Capture::Idle),
        MouseRoute::Bar
    );
    for (button, action) in [
        (MouseButton::Left, MouseAction::Release),
        (MouseButton::NoButton, MouseAction::Motion),
        (MouseButton::Right, MouseAction::Press),
        (MouseButton::Middle, MouseAction::Press),
        (MouseButton::WheelUp, MouseAction::Press),
    ] {
        assert_eq!(
            route_mouse(true, button, action, Capture::Idle),
            MouseRoute::Drop
        );
    }
}

#[test]
fn route_mouse_table() {
    for (in_bar, button, action, held, route) in [
        (
            false,
            MouseButton::Left,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::NoButton,
            MouseAction::Motion,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            false,
            MouseButton::WheelDown,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Bar,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Motion,
            Capture::Server,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::Left,
            MouseAction::Release,
            Capture::Server,
            MouseRoute::Server,
        ),
        (
            true,
            MouseButton::WheelUp,
            MouseAction::Press,
            Capture::Idle,
            MouseRoute::Drop,
        ),
    ] {
        assert_eq!(
            route_mouse(in_bar, button, action, held),
            route,
            "{in_bar} {button:?} {action:?} {held:?}"
        );
    }
}

#[test]
fn pane_drag_keeps_reaching_the_server_over_the_bar() {
    let mut capture = Capture::Idle;
    let mut routes = Vec::new();
    for (in_bar, button, action) in [
        (false, MouseButton::Left, MouseAction::Press),
        (true, MouseButton::Left, MouseAction::Motion),
        (true, MouseButton::Left, MouseAction::Release),
        (true, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(in_bar, button, action, capture);
        capture = capture_after(capture, button, action, route);
        routes.push(route);
    }
    assert_eq!(
        routes,
        [
            MouseRoute::Server,
            MouseRoute::Server,
            MouseRoute::Server,
            MouseRoute::Drop
        ]
    );
    assert_eq!(capture, Capture::Idle);
}

#[test]
fn tab_drag_into_a_pane_never_reaches_the_server() {
    let mut capture = Capture::Idle;
    let mut routes = Vec::new();
    for (in_bar, button, action) in [
        (true, MouseButton::Left, MouseAction::Press),
        (true, MouseButton::Left, MouseAction::Motion),
        (false, MouseButton::Left, MouseAction::Motion),
        (false, MouseButton::Left, MouseAction::Release),
        (false, MouseButton::NoButton, MouseAction::Motion),
    ] {
        let route = route_mouse(in_bar, button, action, capture);
        capture = capture_after(capture, button, action, route);
        routes.push(route);
    }
    assert_eq!(
        routes,
        [
            MouseRoute::Bar,
            MouseRoute::Drop,
            MouseRoute::Drop,
            MouseRoute::Drop,
            MouseRoute::Server
        ]
    );
    assert_eq!(capture, Capture::Idle);
}

#[test]
fn wheel_presses_do_not_start_a_capture_and_bar_presses_capture_the_bar() {
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::WheelUp,
            MouseAction::Press,
            MouseRoute::Server
        ),
        Capture::Idle
    );
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::Left,
            MouseAction::Press,
            MouseRoute::Bar
        ),
        Capture::Bar
    );
    assert_eq!(
        capture_after(
            Capture::Idle,
            MouseButton::Right,
            MouseAction::Press,
            MouseRoute::Server
        ),
        Capture::Server
    );
}

#[test]
fn bar_hits_map_to_commands() {
    assert_eq!(
        bar_command(HitTarget::Window(flair_core::ids::WindowId(4))),
        Command::SelectWindow(WindowRef::Id(flair_core::ids::WindowId(4)))
    );
    assert_eq!(bar_command(HitTarget::Session), Command::ChooseSession);
}

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
