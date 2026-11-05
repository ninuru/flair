use flair_core::{
    ids::{ClientId, PaneId},
    proto::ClientMsg,
};

#[derive(Debug)]
pub enum Event {
    ClientConnected(ClientId),
    Client(ClientId, ClientMsg),
    ClientGone(ClientId),
    PtyOutput(PaneId, Vec<u8>),
    PtyExited(PaneId),
    PtySpawnFailed(PaneId),
    Timer(u64),
}
