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
    thread::Builder::new()
        .name("flair-state".into())
        .spawn(move || run(config, link))
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
    fn spawn(id: PaneId, spec: &SpawnSpec, events: SyncSender<Event>) -> io::Result<Self> {
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
    events: SyncSender<Event>,
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

fn send_event(events: &SyncSender<Event>, stop: &AtomicBool, mut event: Event) -> bool {
    while !stop.load(Ordering::Acquire) {
        match events.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Full(pending)) => {
                event = pending;
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

fn run(config: ServerConfig, link: ServerLink) {
    let started = Instant::now();
    let mut server = Server::new(config);
    let (event_tx, event_rx) = mpsc::sync_channel(32);
    let mut worker: Option<PtyWorker> = None;
    let mut timers = Vec::new();
    let mut shutdown = false;
    while !shutdown {
        let now = Millis(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
        let mut effects = Vec::new();
        for _ in 0..64 {
            match link.rx.try_recv() {
                Ok(message) => {
                    server.handle(now, Event::Client(ClientId(1), message), &mut effects)
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    shutdown = true;
                    break;
                }
            }
        }
        for _ in 0..64 {
            match event_rx.try_recv() {
                Ok(event) => server.handle(now, event, &mut effects),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
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
        for effect in effects {
            match effect {
                Effect::Spawn(id, spec) => match PtyWorker::spawn(id, &spec, event_tx.clone()) {
                    Ok(pty) => worker = Some(pty),
                    Err(error) => {
                        let _ = link
                            .tx
                            .send(ServerMsg::Error(format!("could not start shell: {error}")));
                        let _ = link.tx.send(ServerMsg::Exit(ExitReason::ServerExited));
                        shutdown = true;
                    }
                },
                Effect::WritePty(_, bytes) => {
                    if let Some(worker) = &worker
                        && worker.writer.send(bytes).is_err()
                    {
                        shutdown = true;
                    }
                }
                Effect::ResizePty(_, size) => {
                    if let Some(worker) = &mut worker {
                        worker.pty.resize(size);
                    }
                }
                Effect::KillPty(_) => worker = None,
                Effect::Send(_, message) => {
                    if link.tx.send(message).is_err() {
                        shutdown = true;
                    }
                }
                Effect::SetTimer(id, when) => timers.push((id, when)),
                Effect::Shutdown => shutdown = true,
            }
        }
        if let Some(worker) = &mut worker {
            worker.poll_exit();
        }
        if !shutdown {
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
