use crate::{config::ServerConfig, effect::Effect, event::Event, server::Server};
use flair_core::{
    ids::{ClientId, PaneId},
    link::ServerLink,
    proto::{ExitReason, ServerMsg},
    time::Millis,
};
use flair_term::pty::{Pty, SpawnSpec};
use std::{
    fs::File,
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub fn spawn_in_process(config: ServerConfig, link: ServerLink) -> io::Result<JoinHandle<()>> {
    let (events, rx) = mpsc::sync_channel(128);
    let (output, outgoing) = mpsc::sync_channel(32);
    let input_events = events.clone();
    thread::Builder::new()
        .name("flair-local-input".into())
        .spawn(move || {
            for message in link.rx {
                if input_events
                    .send(RuntimeEvent::State(Event::Client(ClientId(1), message)))
                    .is_err()
                {
                    return;
                }
            }
            let _ = input_events.send(RuntimeEvent::Stop);
        })?;
    thread::Builder::new()
        .name("flair-local-output".into())
        .spawn(move || {
            for message in outgoing {
                if link.tx.send(message).is_err() {
                    break;
                }
            }
        })?;
    events
        .send(RuntimeEvent::Connect(ClientId(1), output))
        .map_err(io::Error::other)?;
    thread::Builder::new()
        .name("flair-state".into())
        .spawn(move || run(config, events, rx, true))
}

enum RuntimeEvent {
    State(Event),
    Connect(ClientId, SyncSender<ServerMsg>),
    Stop,
}

static TERMINATED: AtomicBool = AtomicBool::new(false);
extern "C" fn terminate(_: libc::c_int) {
    TERMINATED.store(true, Ordering::Release);
}

pub fn serve(config: ServerConfig, path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixListener,
    };
    crate::socket::private_parent(path)?;
    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "server already running",
            ));
        }
        let meta = std::fs::symlink_metadata(path)?;
        if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::other(
                "refusing to replace non-socket or foreign path",
            ));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    TERMINATED.store(false, Ordering::Release);
    unsafe {
        libc::signal(libc::SIGTERM, terminate as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, terminate as *const () as libc::sighandler_t);
    }
    let (events, rx) = mpsc::sync_channel(128);
    let stop = Arc::new(AtomicBool::new(false));
    let listener_stop = stop.clone();
    let listener_events = events.clone();
    let acceptor = thread::Builder::new()
        .name("flair-listener".into())
        .spawn(move || {
            let mut next = 1u32;
            while !listener_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let id = ClientId(next);
                        let Some(value) = next.checked_add(1) else {
                            break;
                        };
                        next = value;
                        let events = listener_events.clone();
                        if let Err(error) = thread::Builder::new()
                            .name("flair-socket".into())
                            .spawn(move || {
                                socket_client(id, stream, events);
                            })
                        {
                            log::error!("could not accept client: {error}");
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => {
                        log::error!("socket accept failed: {error}");
                        break;
                    }
                }
            }
        })?;
    run(config, events, rx, false);
    stop.store(true, Ordering::Release);
    if acceptor.join().is_err() {
        log::error!("socket listener panicked");
    }
    std::fs::remove_file(path)
}

fn socket_client(
    id: ClientId,
    mut stream: std::os::unix::net::UnixStream,
    events: SyncSender<RuntimeEvent>,
) {
    if crate::socket::handshake(&mut stream).is_err() {
        return;
    }
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    if writer
        .set_write_timeout(Some(Duration::from_secs(2)))
        .is_err()
    {
        return;
    }
    let (tx, rx) = mpsc::sync_channel(32);
    if events.send(RuntimeEvent::Connect(id, tx)).is_err() {
        return;
    }
    let writer_events = events.clone();
    let output = thread::Builder::new()
        .name("flair-socket-writer".into())
        .spawn(move || {
            for message in rx {
                if crate::socket::write_message(&mut writer, &message).is_err() {
                    break;
                }
            }
            let _ = writer.shutdown(std::net::Shutdown::Both);
            let _ = writer_events.send(RuntimeEvent::State(Event::ClientGone(id)));
        });
    if output.is_err() {
        let _ = events.send(RuntimeEvent::State(Event::ClientGone(id)));
        return;
    }
    while let Ok(message) = crate::socket::read_message(&mut stream) {
        if events
            .send(RuntimeEvent::State(Event::Client(id, message)))
            .is_err()
        {
            break;
        }
    }
    let _ = events.send(RuntimeEvent::State(Event::ClientGone(id)));
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

struct PtyWorker {
    pty: Pty,
    writer: Sender<Vec<u8>>,
    stop: Arc<AtomicBool>,
    child_exited: Arc<AtomicBool>,
    reader_handle: Option<JoinHandle<()>>,
    writer_handle: Option<JoinHandle<()>>,
}

impl PtyWorker {
    fn spawn(id: PaneId, spec: &SpawnSpec, events: SyncSender<RuntimeEvent>) -> io::Result<Self> {
        let mut pty = Pty::spawn(spec)?;
        let reader = pty.reader()?;
        let writer = pty.writer()?;
        let stop = Arc::new(AtomicBool::new(false));
        let child_exited = Arc::new(AtomicBool::new(false));
        let reader_stop = stop.clone();
        let reader_exited = child_exited.clone();
        let reader_handle = match thread::Builder::new()
            .name("flair-pty-reader".into())
            .spawn(move || read_pty(id, reader, events, reader_stop, reader_exited))
        {
            Ok(handle) => handle,
            Err(error) => {
                pty.kill();
                return Err(error);
            }
        };
        let (writer_tx, writer_rx) = mpsc::channel();
        let writer_stop = stop.clone();
        let writer_handle = match thread::Builder::new()
            .name("flair-pty-writer".into())
            .spawn(move || write_pty(writer, writer_rx, writer_stop))
        {
            Ok(handle) => handle,
            Err(error) => {
                stop.store(true, Ordering::Release);
                pty.kill();
                let _ = reader_handle.join();
                return Err(error);
            }
        };
        Ok(Self {
            pty,
            writer: writer_tx,
            stop,
            child_exited,
            reader_handle: Some(reader_handle),
            writer_handle: Some(writer_handle),
        })
    }

    fn poll_exit(&mut self) {
        match self.pty.try_wait() {
            Ok(Some(_)) => self.child_exited.store(true, Ordering::Release),
            Ok(None) => {}
            Err(error) => {
                log::error!("failed to inspect PTY child: {error}");
                self.child_exited.store(true, Ordering::Release);
            }
        }
    }
}

impl Drop for PtyWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.pty.kill();
        for handle in [self.reader_handle.take(), self.writer_handle.take()]
            .into_iter()
            .flatten()
        {
            if handle.join().is_err() {
                log::error!("PTY worker panicked during shutdown");
            }
        }
    }
}

fn read_pty(
    id: PaneId,
    mut reader: File,
    events: SyncSender<RuntimeEvent>,
    stop: Arc<AtomicBool>,
    child_exited: Arc<AtomicBool>,
) {
    let mut buffer = [0; 65536];
    while !stop.load(Ordering::Acquire) {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(len) => {
                if !send_event(&events, &stop, Event::PtyOutput(id, buffer[..len].to_vec())) {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child_exited.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                if error.raw_os_error() != Some(5) {
                    log::error!("PTY read failed: {error}");
                }
                break;
            }
        }
    }
    send_event(&events, &stop, Event::PtyExited(id));
}

fn send_event(events: &SyncSender<RuntimeEvent>, stop: &AtomicBool, mut event: Event) -> bool {
    while !stop.load(Ordering::Acquire) {
        match events.try_send(RuntimeEvent::State(event)) {
            Ok(()) => return true,
            Err(TrySendError::Full(pending)) => {
                if let RuntimeEvent::State(pending) = pending {
                    event = pending;
                } else {
                    return false;
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
    false
}

fn write_pty(mut writer: File, queue: Receiver<Vec<u8>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        let bytes = match queue.recv_timeout(Duration::from_millis(10)) {
            Ok(bytes) => bytes,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut remaining = bytes.as_slice();
        while !remaining.is_empty() && !stop.load(Ordering::Acquire) {
            match writer.write(remaining) {
                Ok(0) => return,
                Ok(len) => remaining = &remaining[len..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(error) => {
                    log::error!("PTY write failed: {error}");
                    return;
                }
            }
        }
    }
}

fn run(
    config: ServerConfig,
    event_tx: SyncSender<RuntimeEvent>,
    event_rx: Receiver<RuntimeEvent>,
    owned: bool,
) {
    use std::collections::HashMap;
    let started = Instant::now();
    let mut server = Server::new(config);
    let mut workers: HashMap<PaneId, PtyWorker> = HashMap::new();
    let mut clients: HashMap<ClientId, SyncSender<ServerMsg>> = HashMap::new();
    let mut timers = Vec::new();
    let mut shutdown = false;
    let mut pending_effects = Vec::new();
    while !shutdown && (owned || !TERMINATED.load(Ordering::Acquire)) {
        let now = Millis(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
        let mut effects = std::mem::take(&mut pending_effects);
        let mut spawn_owners = HashMap::new();
        for _ in 0..128 {
            match event_rx.try_recv() {
                Ok(RuntimeEvent::Connect(id, sender)) => {
                    clients.insert(id, sender);
                    server.handle(now, Event::ClientConnected(id), &mut effects);
                }
                Ok(RuntimeEvent::State(event)) => {
                    if let Event::ClientGone(id) = &event {
                        clients.remove(id);
                    }
                    let owner = if let Event::Client(id, _) = &event {
                        Some(*id)
                    } else {
                        None
                    };
                    let first = effects.len();
                    server.handle(now, event, &mut effects);
                    if let Some(owner) = owner {
                        for effect in &effects[first..] {
                            if let Effect::Spawn(pane, _) = effect {
                                spawn_owners.insert(*pane, owner);
                            }
                        }
                    }
                }
                Ok(RuntimeEvent::Stop) => {
                    shutdown = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    shutdown = true;
                    break;
                }
            }
        }
        let mut pending_timers = Vec::new();
        for (id, when) in timers.drain(..) {
            if when <= now {
                server.handle(now, Event::Timer(id), &mut effects);
            } else {
                pending_timers.push((id, when));
            }
        }
        timers = pending_timers;
        let mut gone = Vec::new();
        let mut failed = Vec::new();
        for effect in effects {
            match effect {
                Effect::Spawn(id, spec) => match PtyWorker::spawn(id, &spec, event_tx.clone()) {
                    Ok(pty) => {
                        workers.insert(id, pty);
                    }
                    Err(error) => {
                        if let Some(sender) =
                            spawn_owners.get(&id).and_then(|owner| clients.get(owner))
                        {
                            let _ = sender.try_send(ServerMsg::Error(format!(
                                "could not start shell: {error}"
                            )));
                        }
                        failed.push(id);
                    }
                },
                Effect::WritePty(id, bytes) => {
                    if let Some(worker) = workers.get(&id) {
                        let _ = worker.writer.send(bytes);
                    }
                }
                Effect::ResizePty(id, size) => {
                    if let Some(worker) = workers.get_mut(&id) {
                        worker.pty.resize(size);
                    }
                }
                Effect::KillPty(id) => {
                    workers.remove(&id);
                }
                Effect::Send(id, message) => {
                    let exit = matches!(message, ServerMsg::Exit(_));
                    if let Some(sender) = clients.get(&id)
                        && (sender.try_send(message).is_err() || exit)
                    {
                        gone.push(id);
                    }
                }
                Effect::SetTimer(id, when) => timers.push((id, when)),
                Effect::Shutdown => shutdown = true,
            }
        }
        for id in gone {
            clients.remove(&id);
            server.handle(now, Event::ClientGone(id), &mut pending_effects);
        }
        for id in failed {
            server.handle(now, Event::PtySpawnFailed(id), &mut pending_effects);
        }
        for worker in workers.values_mut() {
            worker.poll_exit();
        }
        if !shutdown {
            thread::sleep(Duration::from_millis(1));
        }
    }
    for sender in clients.values() {
        let _ = sender.try_send(ServerMsg::Exit(ExitReason::ServerExited));
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
