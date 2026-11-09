use std::{
    fs,
    path::PathBuf,
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
fn status_names_windows_after_directory_and_command_at_any_width() {
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
        assert!(status.starts_with(" 1:flair 2:htop "), "{status:?}");
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
