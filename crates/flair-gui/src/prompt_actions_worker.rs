use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::context_names;

pub(crate) fn docker_contexts() -> Result<Vec<String>, String> {
    let output = docker(&["context", "ls", "--format", "{{.Name}}"])?;
    let names = context_names(&output);
    if names.is_empty() {
        Err("No Docker contexts found".into())
    } else {
        Ok(names)
    }
}

pub(crate) fn switch_docker_context(name: &str) -> Result<(), String> {
    if context_names(name) != [name] {
        return Err("Invalid Docker context name".into());
    }
    if std::env::var_os("DOCKER_CONTEXT").is_some_and(|value| !value.is_empty()) {
        return Err(
            "DOCKER_CONTEXT overrides the active context; unset it before switching".into(),
        );
    }
    docker(&["context", "use", name]).map(|_| ())
}

fn docker(args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("docker");
    command.args(args);
    run(command)
}

fn run(mut command: Command) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Cannot start Docker: {error}"))?;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or("Cannot capture Docker output")?;
        let fd = stdout.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(format!(
                "Cannot read Docker output: {}",
                io::Error::last_os_error()
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut output = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let mut eof = false;
            for _ in 0..17 {
                match stdout.read(&mut buffer) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(count) => {
                        if output.len() + count > 65536 {
                            return Err("Docker context output is too large".into());
                        }
                        output.extend_from_slice(&buffer[..count]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(format!("Cannot read Docker contexts: {error}")),
                }
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("Cannot wait for Docker: {error}"))?
            {
                if !status.success() {
                    return Err("Docker context command failed; check Docker configuration".into());
                }
                if eof {
                    return String::from_utf8(output)
                        .map_err(|error| format!("Invalid Docker output: {error}"));
                }
            }
            if Instant::now() >= deadline {
                return Err("Docker context command timed out".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
#[path = "prompt_actions_worker_tests.rs"]
mod tests;
