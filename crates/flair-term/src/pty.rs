use crate::shell_integration::ShellIntegration;
use alacritty_terminal::{
    event::{OnResize, WindowSize},
    tty::{self, Options, Shell},
};
use flair_core::geom::GridSize;
use std::{
    fs::File,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub unset_env: Vec<String>,
    pub size: GridSize,
}
pub struct Pty {
    inner: Option<tty::Pty>,
    pid: u32,
    status: Option<i32>,
    integration: Option<ShellIntegration>,
}
impl Pty {
    pub fn spawn(spec: &SpawnSpec) -> io::Result<Pty> {
        if spec.size.cols == 0 || spec.size.rows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PTY dimensions must be nonzero",
            ));
        }
        let integration = ShellIntegration::prepare(spec)?;
        let integration_env = integration
            .as_ref()
            .map(|integration| integration.env(spec))
            .transpose()?
            .unwrap_or_default();
        let spawn = SpawnSpec {
            args: integration.as_ref().map_or_else(
                || spec.args.clone(),
                |integration| integration.args(spec, &integration_env),
            ),
            env: spec.env.iter().cloned().chain(integration_env).collect(),
            unset_env: spec
                .unset_env
                .iter()
                .filter(|name| integration.is_none() || *name != "ZDOTDIR")
                .cloned()
                .collect(),
            ..spec.clone()
        };
        let spec = &spawn;
        let shell = if spec.unset_env.is_empty() {
            Shell::new(spec.program.clone(), spec.args.clone())
        } else {
            let program = std::path::Path::new(&spec.program);
            let candidates = if program.components().count() > 1 || program.is_absolute() {
                vec![spec.cwd.join(program)]
            } else {
                let path = spec
                    .env
                    .iter()
                    .rev()
                    .find(|(key, _)| key == "PATH")
                    .map(|(_, value)| std::ffi::OsString::from(value))
                    .or_else(|| std::env::var_os("PATH"))
                    .unwrap_or_else(|| "/bin:/usr/bin".into());
                std::env::split_paths(&path)
                    .map(|path| spec.cwd.join(path).join(program))
                    .collect()
            };
            if !candidates.iter().any(|path| {
                path.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("shell is not executable: {}", spec.program),
                ));
            }
            let args = spec
                .unset_env
                .iter()
                .flat_map(|name| ["-u".to_owned(), name.clone()])
                .chain(["--".to_owned(), spec.program.clone()])
                .chain(spec.args.iter().cloned())
                .collect();
            Shell::new("/usr/bin/env".to_owned(), args)
        };
        if !spec.cwd.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("session directory does not exist: {}", spec.cwd.display()),
            ));
        }
        let config = Options {
            shell: Some(shell),
            working_directory: Some(spec.cwd.clone()),
            drain_on_exit: true,
            env: spec
                .env
                .iter()
                .cloned()
                .chain([("TERM_PROGRAM".to_owned(), "flair".to_owned())])
                .collect(),
        };
        let inner = tty::new(&config, window_size(spec.size), 0)?;
        let pid = inner.child().id();
        Ok(Pty {
            inner: Some(inner),
            pid,
            status: None,
            integration,
        })
    }
    pub fn reader(&self) -> io::Result<File> {
        self.file()?.try_clone()
    }
    pub fn writer(&self) -> io::Result<File> {
        self.file()?.try_clone()
    }
    fn file(&self) -> io::Result<&File> {
        self.inner
            .as_ref()
            .map(|p| p.file())
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "PTY has exited"))
    }
    pub fn resize(&mut self, size: GridSize) {
        if let Some(pty) = &mut self.inner {
            pty.on_resize(window_size(size));
        }
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn foreground(&self) -> Option<u32> {
        let pgid = unsafe { libc::tcgetpgrp(self.file().ok()?.as_raw_fd()) };
        u32::try_from(pgid).ok().filter(|pgid| *pgid > 0)
    }
    pub fn cwd(&self) -> io::Result<PathBuf> {
        let pgid = unsafe { libc::tcgetpgrp(self.file()?.as_raw_fd()) };
        if pgid > 0 {
            std::fs::read_link(format!("/proc/{pgid}/cwd"))
                .or_else(|_| std::fs::read_link(format!("/proc/{}/cwd", self.pid)))
        } else {
            std::fs::read_link(format!("/proc/{}/cwd", self.pid))
        }
    }
    pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
        if self.status.is_none() && self.inner.is_some() {
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            let info = unsafe { info.assume_init() };
            if unsafe { info.si_pid() } != 0 {
                self.status = Some(if info.si_code == libc::CLD_EXITED {
                    unsafe { info.si_status() }
                } else {
                    -1
                });
            }
        }
        Ok(self.status)
    }
    pub fn kill(&mut self) {
        if let Some(pty) = self.inner.take() {
            match std::fs::read_dir("/proc") {
                Ok(processes) => {
                    for entry in processes.filter_map(Result::ok) {
                        let Some(pid) = entry
                            .file_name()
                            .to_str()
                            .and_then(|name| name.parse::<i32>().ok())
                        else {
                            continue;
                        };
                        if pid == self.pid as i32 || unsafe { libc::getsid(pid) } != self.pid as i32
                        {
                            continue;
                        }
                        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
                        if fd < 0 {
                            let error = io::Error::last_os_error();
                            if error.raw_os_error() != Some(libc::ESRCH) {
                                log::warn!("Could not open PTY session process handle: {error}");
                            }
                            continue;
                        }
                        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
                        if unsafe { libc::getsid(pid) } == self.pid as i32 {
                            unsafe {
                                libc::syscall(
                                    libc::SYS_pidfd_send_signal,
                                    fd.as_raw_fd(),
                                    libc::SIGKILL,
                                    std::ptr::null::<libc::siginfo_t>(),
                                    0,
                                );
                            }
                        }
                    }
                }
                Err(error) => log::warn!("Could not enumerate PTY session processes: {error}"),
            }
            unsafe {
                libc::kill(-(self.pid as i32), libc::SIGKILL);
                if self.status.is_none() {
                    libc::kill(self.pid as i32, libc::SIGKILL);
                }
            }
            drop(pty);
            self.integration.take();
        }
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        self.kill();
    }
}
fn window_size(size: GridSize) -> WindowSize {
    WindowSize {
        num_cols: size.cols.max(1),
        num_lines: size.rows.max(1),
        cell_width: 1,
        cell_height: 1,
    }
}

#[cfg(test)]
#[path = "pty_tests.rs"]
mod tests;
