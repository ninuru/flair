use flair_core::{
    codec::{self, MAX_FRAME},
    link::ClientLink,
    proto::{ClientMsg, PROTOCOL, ServerMsg},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            net::UnixStream,
        },
    },
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

pub fn socket_path() -> io::Result<PathBuf> {
    resolve_socket_path(
        env::var_os("FLAIR_SOCKET"),
        env::var_os("FLAIR_SOCKET_NAME"),
        env::var_os("FLAIR"),
        env::var_os("XDG_RUNTIME_DIR"),
    )
}

fn resolve_socket_path(
    explicit: Option<OsString>,
    name: Option<OsString>,
    inherited: Option<OsString>,
    runtime: Option<OsString>,
) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(PathBuf::from(path));
    }
    if name.is_none()
        && let Some(value) = inherited
    {
        let value = value.to_string_lossy();
        if let Some((path, _)) = value.rsplit_once(',') {
            return Ok(PathBuf::from(path));
        }
    }
    let name = name
        .unwrap_or_else(|| "default".into())
        .into_string()
        .map_err(|_| io::Error::other("invalid FLAIR_SOCKET_NAME"))?;
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(io::Error::other("invalid FLAIR_SOCKET_NAME"));
    }
    let root = runtime
        .map(|path| PathBuf::from(path).join("flair"))
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/flair-{}", unsafe { libc::geteuid() })));
    Ok(root.join(format!("{name}.sock")))
}

pub fn private_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| io::Error::other("socket must have a parent directory"))?;
    match fs::DirBuilder::new().mode(0o700).create(parent) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::other(
            "socket directory must be owned by this user with mode 0700",
        ));
    }
    Ok(())
}

pub fn lock(path: &Path) -> io::Result<fs::File> {
    private_parent(path)?;
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(PathBuf::from(name))?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::other("unsafe socket lock file"));
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

pub fn peer_pid(stream: &UnixStream) -> io::Result<i32> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if cred.uid != unsafe { libc::geteuid() } || cred.pid <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket peer is not this user",
        ));
    }
    Ok(cred.pid)
}

pub fn handshake(stream: &mut UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    peer_pid(stream)?;
    stream.write_all(&PROTOCOL.to_le_bytes())?;
    let mut version = [0; 4];
    stream.read_exact(&mut version)?;
    let version = u32::from_le_bytes(version);
    if version != PROTOCOL {
        return Err(io::Error::other(format!(
            "server speaks protocol {version}, this binary speaks {PROTOCOL}; run flair kill-server"
        )));
    }
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)
}

pub fn write_message<T: Serialize>(stream: &mut UnixStream, message: &T) -> io::Result<()> {
    let mut bytes = Vec::new();
    codec::encode(message, &mut bytes).map_err(io::Error::other)?;
    stream.write_all(&bytes)
}

pub fn read_message<T: DeserializeOwned>(stream: &mut UnixStream) -> io::Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::other("socket frame exceeds protocol limit"));
    }
    let mut reader = codec::FrameReader::default();
    reader.push(&header).map_err(io::Error::other)?;
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    reader.push(&bytes).map_err(io::Error::other)?;
    reader
        .next_frame()
        .ok_or_else(|| io::Error::other("incomplete frame"))?
        .map_err(io::Error::other)
}

pub fn connect(path: &Path) -> io::Result<ClientLink> {
    let mut stream = UnixStream::connect(path)?;
    handshake(&mut stream)?;
    let mut reader = stream.try_clone()?;
    let (tx, outgoing) = mpsc::channel::<ClientMsg>();
    let (incoming, rx) = mpsc::channel::<ServerMsg>();
    thread::Builder::new()
        .name("flair-client-writer".into())
        .spawn(move || {
            for message in outgoing {
                if write_message(&mut stream, &message).is_err() {
                    break;
                }
            }
            let _ = stream.shutdown(std::net::Shutdown::Both);
        })?;
    thread::Builder::new()
        .name("flair-client-reader".into())
        .spawn(move || {
            while let Ok(message) = read_message(&mut reader) {
                if incoming.send(message).is_err() {
                    break;
                }
            }
            let _ = reader.shutdown(std::net::Shutdown::Both);
        })?;
    Ok(ClientLink { tx, rx })
}

#[cfg(test)]
#[path = "socket_tests.rs"]
mod tests;
