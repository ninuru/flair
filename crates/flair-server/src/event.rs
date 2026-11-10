use flair_core::{
    ids::{ClientId, PaneId},
    isles::GitInfo,
    proto::ClientMsg,
};
use std::path::PathBuf;

use crate::facts::Contexts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    Prefix { client: ClientId, generation: u64 },
    Prompt,
}

#[derive(Debug)]
pub enum Event {
    ClientConnected(ClientId),
    Client(ClientId, ClientMsg),
    ClientGone(ClientId),
    PtyOutput(PaneId, Vec<u8>),
    PtyExited(PaneId),
    PtySpawnFailed(PaneId),
    Timer(Timer),
    ConfigRead(Result<String, String>),
    GitRead(PathBuf, Option<GitInfo>),
    ContextsRead(Contexts),
}
