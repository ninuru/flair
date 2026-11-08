use super::*;
use std::{
    io::{Read, Write},
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
