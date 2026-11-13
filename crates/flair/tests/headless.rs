use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct OutputDir(PathBuf);
impl OutputDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "flair-e2e-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_scenarios_run_without_display_and_capture_cells() {
    for scenario in ["basic", "resize", "input", "stress"] {
        let output = OutputDir::new();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/scenarios/{scenario}.toml"));
        let run = Command::new("timeout")
            .args([
                "10s",
                env!("CARGO_BIN_EXE_flair"),
                "test-headless",
                "--scenario",
            ])
            .arg(fixture)
            .arg("--output")
            .arg(&output.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{scenario}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report: toml::Value =
            toml::from_str(&fs::read_to_string(output.0.join("result.toml")).unwrap()).unwrap();
        assert_eq!(report["success"].as_bool(), Some(true));
        let screen = fs::read_to_string(output.0.join("screen.txt")).unwrap();
        assert!(screen.contains(match scenario {
            "basic" => "got:ok",
            "resize" => "30 100",
            "input" => "paste:hello world",
            "stress" => "INTERRUPTED",
            _ => unreachable!(),
        }));
    }
}

#[test]
fn windows_scenario_frames_status_split_and_window_switch() {
    let output = OutputDir::new();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/scenarios/windows.toml"))
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let frame = fs::read_to_string(output.0.join("frame.txt")).unwrap();
    let rows: Vec<&str> = frame.lines().collect();
    assert_eq!(rows.len(), 13);
    assert!(rows[0].starts_with(" 1:env 2:env "));
    assert!(rows[0].ends_with("| main "));
    assert!(rows[1..].iter().all(|row| row.chars().nth(39) == Some('│')));
    assert!(
        rows[1..]
            .iter()
            .any(|row| row.trim_end().ends_with("12 40"))
    );
}

#[test]
fn copy_scenario_shows_the_badge_jumps_through_history_and_swallows_keys_until_q() {
    let output = OutputDir::new();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/scenarios/copy.toml"))
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let frame = fs::read_to_string(output.0.join("frame.txt")).unwrap();
    let rows: Vec<&str> = frame.lines().collect();
    assert!(rows[0].ends_with(" COPY"), "{:?}", rows[0]);
    assert!(frame.contains("ready> z"));
    assert!(!frame.contains("xz"));
}

#[test]
fn status_names_windows_after_the_foreground_process_at_any_width() {
    let fixture = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/scenarios/names.toml"),
    )
    .unwrap();
    for (cols, end) in [(80, "| main "), (24, " | main ")] {
        let output = OutputDir::new();
        let scenario = output.0.join("names.toml");
        fs::write(
            &scenario,
            fixture.replace("cols = 80", &format!("cols = {cols}")),
        )
        .unwrap();
        let run = Command::new("timeout")
            .args([
                "10s",
                env!("CARGO_BIN_EXE_flair"),
                "test-headless",
                "--scenario",
            ])
            .arg(scenario)
            .arg("--output")
            .arg(&output.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{cols}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let frame = fs::read_to_string(output.0.join("frame.txt")).unwrap();
        let status = frame.lines().next().unwrap();
        assert_eq!(status.chars().count(), cols);
        assert!(status.starts_with(" 1:sleep 2:cat "), "{status:?}");
        assert!(status.ends_with(end), "{status:?}");
    }
}

#[test]
fn timeout_reports_failure_and_reaps_shell() {
    let output = OutputDir::new();
    let pid_path = output.0.join("shell.pid");
    let scenario = output.0.join("timeout.toml");
    fs::write(
        &scenario,
        format!(
            r#"program = "/bin/sh"
args = ["-c", "echo $$$$ > '{}'; printf waiting; sleep 30"]
[[steps]]
action = "wait"
text = "never-arrives"
"#,
            pid_path.display()
        )
        .replace("$$$$", "$$"),
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--timeout-ms",
            "250",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: toml::Value =
        toml::from_str(&fs::read_to_string(output.0.join("result.toml")).unwrap()).unwrap();
    assert_eq!(report["success"].as_bool(), Some(false));
    assert!(report["error"].as_str().unwrap().contains("deadline"));
    assert!(
        fs::read_to_string(output.0.join("screen.txt"))
            .unwrap()
            .contains("waiting")
    );
    let pid = fs::read_to_string(pid_path).unwrap();
    assert!(
        !PathBuf::from(format!("/proc/{}", pid.trim())).exists(),
        "shell {} survived",
        pid.trim()
    );
}

#[test]
fn invalid_scenario_exits_two_without_starting_shell() {
    let output = OutputDir::new();
    let scenario = output.0.join("invalid.toml");
    fs::write(&scenario, "cols = 0").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_flair"))
        .args(["test-headless", "--scenario"])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
    assert!(!output.0.join("screen.txt").exists());
}

#[test]
fn early_exit_preserves_last_screen_and_reports_failed_step() {
    let output = OutputDir::new();
    let scenario = output.0.join("exit.toml");
    fs::write(
        &scenario,
        r#"program = "/bin/sh"
args = ["-c", "printf final-output"]
[[steps]]
action = "wait"
text = "never-arrives"
"#,
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .output()
        .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: toml::Value =
        toml::from_str(&fs::read_to_string(output.0.join("result.toml")).unwrap()).unwrap();
    assert_eq!(report["failed_step"].as_integer(), Some(1));
    assert!(report["error"].as_str().unwrap().contains("shell exited"));
    assert!(
        fs::read_to_string(output.0.join("screen.txt"))
            .unwrap()
            .contains("final-output")
    );
}

#[test]
fn scenario_shell_does_not_inherit_callers_environment() {
    let output = OutputDir::new();
    let scenario = output.0.join("env.toml");
    fs::write(&scenario, r#"program = "/bin/sh"
args = ["-c", "if test -z \"$FLAIR_TEST_SHOULD_NOT_INHERIT\"; then printf isolated-env; fi; read hold"]
[[steps]]
action = "wait"
text = "isolated-env"
"#).unwrap();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .env("FLAIR_TEST_SHOULD_NOT_INHERIT", "inherited-value")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn pixel_capture_writes_a_ppm_of_the_session_chooser() {
    let output = OutputDir::new();
    let scenario = output.0.join("capture.toml");
    fs::write(
        &scenario,
        r#"program = "/bin/sh"
args = ["-c", "printf 'ready> '; read x"]
[[steps]]
action = "wait"
text = "ready> "
[[steps]]
action = "key"
key = "a"
ctrl = true
[[steps]]
action = "key"
key = "s"
[[steps]]
action = "capture"
name = "chooser"
[[steps]]
action = "key"
key = "l"
[[steps]]
action = "capture"
name = "window"
[[steps]]
action = "key"
key = "/"
[[steps]]
action = "text"
text = "ma"
[[steps]]
action = "capture"
name = "search"
[[steps]]
action = "window"
width = 320
height = 400
[[steps]]
action = "capture"
name = "narrow"
"#,
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "90s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--pixels",
            "640x400",
            "--timeout-ms",
            "60000",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    if !run.status.success() && stderr.contains("capture needs an EGL device") {
        if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
            panic!("required EGL unavailable: {stderr}");
        }
        eprintln!("skipped pixel capture: {stderr}");
        return;
    }
    assert!(run.status.success(), "{stderr}");
    for (name, width, height) in [
        ("chooser", 640, 400),
        ("window", 640, 400),
        ("search", 640, 400),
        ("narrow", 320, 400),
    ] {
        let image = fs::read(output.0.join(format!("{name}.ppm"))).unwrap();
        let header = format!("P6\n{width} {height}\n255\n");
        assert!(image.starts_with(header.as_bytes()), "{name}");
        assert_eq!(image.len(), header.len() + width * height * 3, "{name}");
    }
}

#[test]
fn pixel_capture_writes_the_settings_page() {
    let output = OutputDir::new();
    let config = output.0.join("flair.toml");
    let original = "# kept by the settings page\n[window]\npadding_x = 12 # roomy\n";
    fs::write(&config, original).unwrap();
    let scenario = output.0.join("settings.toml");
    let mut steps = String::from(
        r#"program = "/bin/sh"
args = ["-c", "printf 'ready> '; read x"]
[[steps]]
action = "wait"
text = "ready> "
[[steps]]
action = "key"
key = "a"
ctrl = true
[[steps]]
action = "key"
key = "S"
shift = true
[[steps]]
action = "capture"
name = "settings"
[[steps]]
action = "key"
key = "Tab"
"#,
    );
    for _ in 0..4 {
        steps.push_str("[[steps]]\naction = \"key\"\nkey = \"Down\"\n");
    }
    steps.push_str(
        r#"[[steps]]
action = "key"
key = "Tab"
[[steps]]
action = "key"
key = " "
[[steps]]
action = "capture"
name = "toggled"
[[steps]]
action = "text"
text = "pad"
[[steps]]
action = "key"
key = "Right"
[[steps]]
action = "key"
key = "Enter"
[[steps]]
action = "capture"
name = "search"
[[steps]]
action = "window"
width = 960
height = 600
[[steps]]
action = "capture"
name = "narrow"
"#,
    );
    fs::write(&scenario, steps).unwrap();
    let run = Command::new("timeout")
        .args([
            "90s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--pixels",
            "1280x800",
            "--timeout-ms",
            "60000",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .arg("--config")
        .arg(&config)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    if !run.status.success() && stderr.contains("capture needs an EGL device") {
        if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
            panic!("required EGL unavailable: {stderr}");
        }
        eprintln!("skipped settings pixel capture: {stderr}");
        return;
    }
    assert!(run.status.success(), "{stderr}");
    for (name, width, height) in [
        ("settings", 1280, 800),
        ("toggled", 1280, 800),
        ("search", 1280, 800),
        ("narrow", 960, 600),
    ] {
        let image = fs::read(output.0.join(format!("{name}.ppm"))).unwrap();
        let header = format!("P6\n{width} {height}\n255\n");
        assert!(image.starts_with(header.as_bytes()), "{name}");
        assert_eq!(image.len(), header.len() + width * height * 3, "{name}");
    }
    let settings = fs::read(output.0.join("settings.ppm")).unwrap();
    let toggled = fs::read(output.0.join("toggled.ppm")).unwrap();
    assert_ne!(settings, toggled);
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
}

#[test]
fn a_reopened_settings_page_shows_the_saves_kept_in_memory() {
    let output = OutputDir::new();
    let config = output.0.join("flair.toml");
    let original = "# untouched by headless saves\n";
    fs::write(&config, original).unwrap();
    let scenario = output.0.join("reopen.toml");
    let key = |key: &str| format!("[[steps]]\naction = \"key\"\nkey = \"{key}\"\n");
    let open = format!(
        "[[steps]]\naction = \"key\"\nkey = \"a\"\nctrl = true\n[[steps]]\naction = \"key\"\nkey = \"S\"\nshift = true\n[[steps]]\naction = \"sleep\"\nms = 300\n{}{}{}{}",
        key("Tab"),
        key("Down"),
        key("Down"),
        key("Tab")
    );
    let steps = format!(
        "program = \"/bin/sh\"\nargs = [\"-c\", \"printf 'ready> '; read x\"]\n[[steps]]\naction = \"wait\"\ntext = \"ready> \"\n{open}{}{}{}[[steps]]\naction = \"capture\"\nname = \"saved\"\n{}{open}[[steps]]\naction = \"capture\"\nname = \"reopened\"\n",
        key("Right"),
        key("Enter"),
        key("Up"),
        key("Escape")
    );
    fs::write(&scenario, steps).unwrap();
    let run = Command::new("timeout")
        .args([
            "90s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--pixels",
            "1280x800",
            "--timeout-ms",
            "60000",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .arg("--config")
        .arg(&config)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    if !run.status.success() && stderr.contains("capture needs an EGL device") {
        if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
            panic!("required EGL unavailable: {stderr}");
        }
        eprintln!("skipped settings reopen capture: {stderr}");
        return;
    }
    assert!(run.status.success(), "{stderr}");
    let saved = fs::read(output.0.join("saved.ppm")).unwrap();
    let reopened = fs::read(output.0.join("reopened.ppm")).unwrap();
    assert!(
        saved == reopened,
        "the reopened page differs from the saved one"
    );
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
}

#[test]
fn capture_without_pixels_writes_the_text_frame() {
    let output = OutputDir::new();
    let scenario = output.0.join("capture.toml");
    fs::write(
        &scenario,
        r#"program = "/bin/sh"
args = ["-c", "printf captured; read x"]
[[steps]]
action = "wait"
text = "captured"
[[steps]]
action = "capture"
name = "text"
"#,
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "10s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        fs::read_to_string(output.0.join("text.txt"))
            .unwrap()
            .contains("captured")
    );
}

#[test]
fn prefix_shift_z_grows_the_pane_into_zen_and_restores_it() {
    let output = OutputDir::new();
    let scenario = output.0.join("zen.toml");
    let toggle = r#"[[steps]]
action = "key"
key = "a"
ctrl = true
[[steps]]
action = "key"
key = "Z"
shift = true
[[steps]]
action = "sleep"
ms = 300
"#;
    let report = |marker: &str| {
        format!(
            "[[steps]]\naction = \"text\"\ntext = \"{marker}\"\n[[steps]]\naction = \"key\"\nkey = \"Enter\"\n[[steps]]\naction = \"wait\"\ntext = \"{marker}=\"\n"
        )
    };
    let program = r#"program = "/bin/sh"
args = ["-c", "printf 'start=%s|' \"$(stty size)\"; while read x; do printf '%s=%s|' \"$x\" \"$(stty size)\"; done"]
[[steps]]
action = "wait"
text = "start="
"#;
    fs::write(
        &scenario,
        format!(
            "{program}{toggle}{}{toggle}{}",
            report("on"),
            report("back")
        ),
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "60s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--pixels",
            "960x300",
            "--scale",
            "1",
            "--timeout-ms",
            "30000",
            "--scenario",
        ])
        .arg(scenario)
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let screen = fs::read_to_string(output.0.join("screen.txt")).unwrap();
    let size = |marker: &str| -> (u16, u16) {
        let rest = screen
            .split(&format!("{marker}="))
            .nth(1)
            .and_then(|rest| rest.split('|').next())
            .unwrap_or_else(|| panic!("{marker} missing in {screen}"));
        let mut numbers = rest.split_whitespace().map(|n| n.parse().unwrap());
        (numbers.next().unwrap(), numbers.next().unwrap())
    };
    let (start, on, back) = (size("start"), size("on"), size("back"));
    assert!(on.0 > start.0 && on.1 > start.1, "{start:?} {on:?}");
    assert_eq!(back, start);
}

fn git(repo: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(["-c", "user.name=flair", "-c", "user.email=flair@invalid"])
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", repo)
        .output()
        .is_ok_and(|run| run.status.success())
}

fn band_labels(capture: &str) -> Vec<String> {
    let rows: Vec<_> = capture.lines().collect();
    let input = rows.iter().rposition(|row| row.trim_end() == "❯").unwrap();
    rows[input - 1]
        .trim_end()
        .split("  ")
        .map(str::to_owned)
        .collect()
}

#[test]
fn prompt_band_shows_directory_branch_and_changes_but_no_exit_chip() {
    if !Path::new("/bin/zsh").exists() {
        eprintln!("skipped prompt band e2e: /bin/zsh missing");
        return;
    }
    let output = OutputDir::new();
    fs::set_permissions(&output.0, fs::Permissions::from_mode(0o700)).unwrap();
    let repo = output.0.join("repo");
    let zdotdir = output.0.join("zdot");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&zdotdir).unwrap();
    if !git(&repo, &["init", "-q", "-b", "main"])
        || !git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"])
    {
        eprintln!("skipped prompt band e2e: git unavailable");
        return;
    }
    fs::write(zdotdir.join(".zshrc"), "PS1='user-theme> '\n").unwrap();
    let scenario = output.0.join("band.toml");
    fs::write(
        &scenario,
        format!(
            r#"program = "/bin/zsh"
args = ["-i"]
cwd = "{repo}"
cols = 120
rows = 8
[env]
FLAIR_PROMPT_SEPARATOR = "chevron"
ZDOTDIR = "{zdotdir}"
HOME = "{home}"
[[steps]]
action = "wait"
text = "❯ "
[[steps]]
action = "text"
text = "stty size\r"
[[steps]]
action = "wait"
text = "8 120"
[[steps]]
action = "text"
text = "false\r"
[[steps]]
action = "wait"
text = "❯ false"
[[steps]]
action = "sleep"
ms = 200
[[steps]]
action = "capture"
name = "band"
[[steps]]
action = "text"
text = "touch x y\r"
[[steps]]
action = "sleep"
ms = 1000
[[steps]]
action = "capture"
name = "changes"
"#,
            repo = repo.display(),
            zdotdir = zdotdir.display(),
            home = output.0.display(),
        ),
    )
    .unwrap();
    let run = Command::new("timeout")
        .args([
            "20s",
            env!("CARGO_BIN_EXE_flair"),
            "test-headless",
            "--scenario",
        ])
        .arg(&scenario)
        .arg("--output")
        .arg(&output.0)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let cwd = "~/repo";
    let band = fs::read_to_string(output.0.join("band.txt")).unwrap();
    assert_eq!(band_labels(&band), [cwd, "main"], "{band}");
    let content: Vec<&str> = band.lines().map(str::trim_end).collect();
    let command = content
        .iter()
        .position(|row| *row == "❯ stty size")
        .unwrap();
    let result = content.iter().position(|row| *row == "8 120").unwrap();
    let failed = content.iter().position(|row| *row == "❯ false").unwrap();
    let prompt = content.iter().rposition(|row| *row == "❯").unwrap();
    assert!(
        command < result && result < failed && failed < prompt - 1,
        "{band}"
    );
    assert!(content[prompt - 1].starts_with(cwd), "{band}");
    assert!(!band.contains("user-theme> "), "{band}");
    let changes = fs::read_to_string(output.0.join("changes.txt")).unwrap();
    assert_eq!(band_labels(&changes), [cwd, "main", "2"], "{changes}");
}
