use super::*;
use std::{
    io::{Read, Write},
    os::unix::fs::DirBuilderExt,
    thread,
    time::{Duration, Instant},
};

fn spec(script: &str) -> SpawnSpec {
    SpawnSpec {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        cwd: PathBuf::from("/tmp"),
        unset_env: Vec::new(),
        env: vec![("TERM".into(), "xterm-256color".into())],
        size: GridSize { cols: 80, rows: 24 },
    }
}
fn read_until(file: &mut File, text: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut out = Vec::new();
    while Instant::now() < deadline {
        let mut bytes = [0; 4096];
        match file.read(&mut bytes) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&bytes[..n]);
                if String::from_utf8_lossy(&out).contains(text) {
                    break;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2))
            }
            Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
            Err(e) => panic!("{e}"),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
#[test]
fn real_pty_output_resize_and_cleanup() {
    let mut pty = Pty::spawn(&spec("printf ready; read x; stty size")).unwrap();
    let mut reader = pty.reader().unwrap();
    assert!(read_until(&mut reader, "ready").contains("ready"));
    pty.resize(GridSize {
        cols: 100,
        rows: 40,
    });
    pty.writer().unwrap().write_all(b"ok\n").unwrap();
    assert!(read_until(&mut reader, "40 100").contains("40 100"));
    pty.kill();
    assert!(pty.reader().is_err());
}
#[test]
fn kill_reaps_a_shell_that_ignores_hangup() {
    let mut pty = Pty::spawn(&spec(
        "trap '' HUP; printf ready; while :; do sleep 10; done",
    ))
    .unwrap();
    assert!(read_until(&mut pty.reader().unwrap(), "ready").contains("ready"));
    let pid = pty.pid();
    pty.kill();
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
}

#[test]
fn natural_exit_can_be_observed_and_reaped_once() {
    let mut pty = Pty::spawn(&spec("printf done; exit 7")).unwrap();
    let pid = pty.pid();
    assert!(read_until(&mut pty.reader().unwrap(), "done").contains("done"));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = pty.try_wait().unwrap() {
            assert_eq!(status, 7);
            assert_eq!(pty.try_wait().unwrap(), Some(7));
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, 0);
            break;
        }
        assert!(Instant::now() < deadline, "child exit was not observed");
        thread::sleep(Duration::from_millis(2));
    }
    drop(pty);
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
}

#[test]
fn cleanup_terminates_job_control_background_processes() {
    let mut pty = Pty::spawn(&spec(
        "set -m; sleep 100 & printf 'child=%s\\n' \"$!\"; read x",
    ))
    .unwrap();
    let text = read_until(&mut pty.reader().unwrap(), "\n");
    let pid = text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("child=")
                .and_then(|n| n.parse::<i32>().ok())
        })
        .expect("missing background pid");
    pty.kill();
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"));
        if stat
            .as_ref()
            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
            || stat.as_ref().is_ok_and(|s| {
                s.rsplit_once(')')
                    .is_some_and(|(_, tail)| tail.trim_start().starts_with('Z'))
            })
        {
            break;
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            panic!("background job survived PTY cleanup");
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn unset_environment_removes_inherited_history() {
    if std::env::var_os("FLAIR_TEST_UNSET_ENV").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "pty::tests::unset_environment_removes_inherited_history",
                "--nocapture",
            ])
            .env("FLAIR_TEST_UNSET_ENV", "1")
            .env("HISTFILE", "/tmp/flair-history-must-not-be-used")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let mut spawn = spec("printf 'history=%s\n' \"${HISTFILE+set}\"; read x");
    spawn
        .env
        .push(("HISTFILE".into(), "configured-history".into()));
    spawn.unset_env.push("HISTFILE".into());
    let mut pty = Pty::spawn(&spawn).unwrap();
    let text = read_until(&mut pty.reader().unwrap(), "\n");
    assert!(text.contains("history=\r\n"), "{text:?}");
    assert!(!text.contains("history=set"), "{text:?}");
    pty.kill();
}

#[test]
fn foreground_cwd_tracks_shell_directory_without_osc7() {
    if !std::path::Path::new("/bin/sh").exists() {
        eprintln!("skipping cwd PTY test: /bin/sh unavailable");
        return;
    }
    let mut pty = Pty::spawn(&spec("cd /; printf ready; read x")).unwrap();
    assert!(read_until(&mut pty.reader().unwrap(), "ready").contains("ready"));
    assert_eq!(pty.cwd().unwrap(), PathBuf::from("/"));
    pty.kill();
    assert!(pty.cwd().is_err());
}

#[test]
fn foreground_job_cwd_takes_precedence_over_shell_cwd() {
    if !std::path::Path::new("/bin/sh").exists() {
        eprintln!("skipping foreground cwd PTY test: /bin/sh unavailable");
        return;
    }
    let mut pty = Pty::spawn(&spec("set -m; (cd /; printf child-ready; read x)")).unwrap();
    assert!(read_until(&mut pty.reader().unwrap(), "child-ready").contains("child-ready"));
    assert_eq!(pty.cwd().unwrap(), PathBuf::from("/"));
    assert_eq!(
        std::fs::read_link(format!("/proc/{}/cwd", pty.pid())).unwrap(),
        PathBuf::from("/tmp")
    );
    pty.kill();
}

#[test]
fn the_foreground_process_group_leader_is_the_running_job() {
    if !std::path::Path::new("/bin/sh").exists() {
        eprintln!("skipping foreground PTY test: /bin/sh unavailable");
        return;
    }
    let mut pty = Pty::spawn(&spec("printf ready; read x")).unwrap();
    assert!(read_until(&mut pty.reader().unwrap(), "ready").contains("ready"));
    assert_eq!(pty.foreground(), Some(pty.pid()));
    pty.kill();
    let mut pty = Pty::spawn(&spec("set -m; (cd /; printf child-ready; read x)")).unwrap();
    assert!(read_until(&mut pty.reader().unwrap(), "child-ready").contains("child-ready"));
    let job = pty.foreground().unwrap();
    assert_ne!(job, pty.pid());
    assert_eq!(
        std::fs::read_link(format!("/proc/{job}/cwd")).unwrap(),
        PathBuf::from("/")
    );
    pty.kill();
}

#[test]
fn zsh_chips_loads_integration_after_user_startup_without_dotfile_changes() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping automatic shell integration test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    std::fs::write(root.join(".zshenv"), "export FLAIR_ENV_LOADED=yes\n").unwrap();
    std::fs::write(root.join(".zprofile"), "export FLAIR_PROFILE_LOADED=yes\n").unwrap();
    std::fs::write(
        root.join(".zlogin"),
        "export FLAIR_LOGIN_LOADED=yes\nPS1='login-prompt> '\n",
    )
    .unwrap();
    let rc = "_FLAIR_INTEGRATION_LOADED=1\nPS1='original-prompt> '\nprecmd() { PS1='framework-prompt> '; RPS1='framework-right-prompt'; }\n";
    std::fs::write(root.join(".zshrc"), rc).unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["-il".into()];
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "chips".into()),
        ("FLAIR_PROMPT_SEPARATOR".into(), "chevron".into()),
    ]);
    let mut pty = Pty::spawn(&spawn).unwrap();
    let mut reader = pty.reader().unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(text.contains("\u{1b}]6973;flair;"), "{text:?}");
    assert!(!text.contains("framework-prompt> "), "{text:?}");
    assert!(!text.contains("framework-right-prompt"), "{text:?}");
    assert!(text.contains("\u{1b}]6973;prompt\u{7}"), "{text:?}");
    let mut terminal = crate::Terminal::new(spawn.size, 100);
    terminal.advance(text.as_bytes());
    let frame = terminal.snapshot(flair_core::ids::PaneId(1), true).unwrap();
    assert_eq!(
        frame.flags.prompt_row,
        Some(frame.cursor.point.row - 1),
        "{text:?}"
    );
    pty.writer().unwrap().write_all(b"printf 'env=%s profile=%s login=%s zdot=%s\\n' \"$FLAIR_ENV_LOADED\" \"$FLAIR_PROFILE_LOADED\" \"$FLAIR_LOGIN_LOADED\" \"$ZDOTDIR\"\n").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(
        text.contains(&format!(
            "env=yes profile=yes login=yes zdot={}",
            root.display()
        )),
        "{text:?}"
    );
    assert_eq!(std::fs::read_to_string(root.join(".zshrc")).unwrap(), rc);
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zsh_ps1_style_preserves_the_user_prompt_and_startup_paths() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping PS1 shell integration test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-ps1-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let integration = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contrib/flair.zsh");
    std::fs::write(
        root.join(".zshrc"),
        format!("PS1='user-prompt> '\nsource '{}'\n", integration.display()),
    )
    .unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["--interactive".into()];
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "ps1".into()),
    ]);
    let mut pty = Pty::spawn(&spawn).unwrap();
    let text = read_until(&mut pty.reader().unwrap(), "user-prompt> ");
    assert!(text.contains("user-prompt> "), "{text:?}");
    assert!(text.contains("\u{1b}]133;A\u{7}"), "{text:?}");
    assert!(!text.contains("\u{1b}]6973;prompt"), "{text:?}");
    assert_eq!(text.matches("\u{1b}]133;B\u{7}").count(), 1, "{text:?}");
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zsh_command_execution_keeps_the_requested_startup_environment() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping zsh command test: /bin/zsh unavailable");
        return;
    }
    let mut spawn = spec("printf 'zdot=%s\\n' \"$ZDOTDIR\"; read x");
    spawn.program = "/bin/zsh".into();
    spawn.env.extend([
        ("ZDOTDIR".into(), "/tmp/flair-command-zdotdir".into()),
        ("FLAIR_PROMPT".into(), "chips".into()),
        ("FLAIR_PROMPT_SEPARATOR".into(), "chevron".into()),
    ]);
    let mut pty = Pty::spawn(&spawn).unwrap();
    let text = read_until(&mut pty.reader().unwrap(), "\n");
    assert!(text.contains("zdot=/tmp/flair-command-zdotdir"), "{text:?}");
    assert!(!text.contains("\u{1b}]6973;"), "{text:?}");
    pty.kill();
}

#[test]
fn zsh_default_prompt_leaves_a_plain_input_line_below_its_chip_anchor() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping plain prompt PTY test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-plain-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["-i".into()];
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "chips".into()),
    ]);
    spawn.unset_env.push("FLAIR_PROMPT_SEPARATOR".into());
    let mut pty = Pty::spawn(&spawn).unwrap();
    let text = read_until(&mut pty.reader().unwrap(), "\u{1b}]133;B\u{7}");
    assert!(!text.contains('❯'), "{text:?}");
    let mut terminal = crate::Terminal::new(spawn.size, 100);
    terminal.advance(text.as_bytes());
    let frame = terminal.snapshot(flair_core::ids::PaneId(1), true).unwrap();
    assert_eq!(frame.cursor.point.row, 1);
    assert_eq!(frame.cursor.point.col, 0);
    assert_eq!(frame.flags.prompt_row, Some(0));
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zsh_ctrl_x_ctrl_e_edits_the_command_line_in_editor_with_the_pane_cwd_and_env() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping edit-command-line PTY test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-edit-{}", std::process::id()));
    let work = root.join("project");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&work)
        .unwrap();
    let editor = root.join("editor");
    std::fs::write(
        &editor,
        "#!/bin/sh\nprintf 'echo edited:%s:%s:%s\\n' \"$(sed 's/ /_/g' \"$1\")\" \"${PWD##*/}\" \"$FLAIR_EDIT_ENV\" > \"$1\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["-i".into()];
    spawn.cwd = work.clone();
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "chips".into()),
        ("FLAIR_PROMPT_SEPARATOR".into(), "chevron".into()),
        ("FLAIR_EDIT_ENV".into(), "pane-env".into()),
        ("EDITOR".into(), editor.to_string_lossy().into_owned()),
    ]);
    spawn.unset_env.push("VISUAL".into());
    let mut pty = Pty::spawn(&spawn).unwrap();
    let mut reader = pty.reader().unwrap();
    read_until(&mut reader, "❯ ");
    let mut writer = pty.writer().unwrap();
    writer.write_all(b"echo original").unwrap();
    read_until(&mut reader, "original");
    writer.write_all(b"\x18\x05").unwrap();
    let text = read_until(&mut reader, "pane-env");
    assert!(
        text.contains("edited:echo_original:project:pane-env"),
        "{text:?}"
    );
    writer.write_all(b"\r").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(
        text.contains("\u{1b}]133;C\u{7}edited:echo_original:project:pane-env\r\n"),
        "{text:?}"
    );
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zsh_empty_enter_starts_a_timed_block_and_only_later_prompts_get_a_gap() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping empty-enter PTY test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-enter-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&root)
        .unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["-i".into()];
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "chips".into()),
        ("FLAIR_PROMPT_SEPARATOR".into(), "chevron".into()),
    ]);
    let mut pty = Pty::spawn(&spawn).unwrap();
    let mut reader = pty.reader().unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(text.contains("\u{1b}]6973;prompt\u{7}"), "{text:?}");
    assert!(!text.contains("\u{1b}]6973;gap"), "{text:?}");
    assert!(!text.contains("\n\u{1b}]6973;prompt"), "{text:?}");
    let mut writer = pty.writer().unwrap();
    writer.write_all(b"\r").unwrap();
    let text = read_until(&mut reader, "❯ ");
    let started = text.find("\u{1b}]133;C\u{7}").unwrap();
    assert!(
        started < text.find("\u{1b}]133;D;0\u{7}").unwrap(),
        "{text:?}"
    );
    assert!(
        text.contains("\u{1b}]6973;gap\u{7}\u{1b}]6973;prompt\u{7}"),
        "{text:?}"
    );
    assert!(!text.contains("\n\u{1b}]6973;prompt"), "{text:?}");
    writer.write_all(b"echo two \\\r").unwrap();
    read_until(&mut reader, "> ");
    writer.write_all(b"\r").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert_eq!(text.matches("\u{1b}]133;C\u{7}").count(), 1, "{text:?}");
    assert!(text.contains("\u{1b}]133;C\u{7}two\r\n"), "{text:?}");
    writer
        .write_all(b"vared -c answer; echo \"[$answer]\"\r")
        .unwrap();
    read_until(&mut reader, "vared");
    thread::sleep(Duration::from_millis(100));
    writer.write_all(b"\r").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(text.contains("[]\r\n"), "{text:?}");
    assert!(!text.contains("\u{1b}]133;C"), "{text:?}");
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zsh_ctrl_c_clears_the_input_in_place_but_still_interrupts_a_command() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("skipping ctrl-c PTY test: /bin/zsh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-zsh-ctrl-c-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&root)
        .unwrap();
    let mut spawn = spec("");
    spawn.program = "/bin/zsh".into();
    spawn.args = vec!["-i".into()];
    spawn.env.extend([
        ("ZDOTDIR".into(), root.to_string_lossy().into_owned()),
        ("HOME".into(), root.to_string_lossy().into_owned()),
        ("FLAIR_PROMPT".into(), "chips".into()),
        ("FLAIR_PROMPT_SEPARATOR".into(), "chevron".into()),
    ]);
    let mut pty = Pty::spawn(&spawn).unwrap();
    let mut reader = pty.reader().unwrap();
    read_until(&mut reader, "❯ ");
    let mut writer = pty.writer().unwrap();
    writer.write_all(b"echo typed").unwrap();
    read_until(&mut reader, "typed");
    writer.write_all(b"\x03").unwrap();
    thread::sleep(Duration::from_millis(100));
    writer.write_all(b"\x03").unwrap();
    thread::sleep(Duration::from_millis(100));
    writer.write_all(b"echo one\r").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(
        !text.contains("\u{1b}]6973;prompt\u{7}\r\n❯ echo typed"),
        "{text:?}"
    );
    assert!(!text.contains("typed\r\n"), "{text:?}");
    assert_eq!(text.matches("\u{1b}]133;C\u{7}").count(), 1, "{text:?}");
    assert_eq!(
        text.matches("\u{1b}]6973;prompt\u{7}").count(),
        1,
        "{text:?}"
    );
    assert!(text.contains("\u{1b}]133;C\u{7}one\r\n"), "{text:?}");
    writer.write_all(b"sleep 5\r").unwrap();
    read_until(&mut reader, "\u{1b}]133;C\u{7}");
    thread::sleep(Duration::from_millis(200));
    writer.write_all(b"\x03").unwrap();
    let text = read_until(&mut reader, "❯ ");
    assert!(text.contains("\u{1b}]133;D;130\u{7}"), "{text:?}");
    pty.kill();
    std::fs::remove_dir_all(root).unwrap();
}
