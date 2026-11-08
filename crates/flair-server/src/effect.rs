use flair_core::{
    geom::GridSize,
    ids::{ClientId, PaneId},
    proto::ServerMsg,
    time::Millis,
};
use flair_term::pty::SpawnSpec;

#[derive(Debug)]
pub enum Effect {
    Spawn(PaneId, SpawnSpec),
    SpawnSplit(PaneId, PaneId, SpawnSpec),
    WritePty(PaneId, Vec<u8>),
    ResizePty(PaneId, GridSize),
    KillPty(PaneId),
    Send(ClientId, ServerMsg),
    SetTimer(u64, Millis),
    Shutdown,
}
