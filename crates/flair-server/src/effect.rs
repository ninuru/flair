use flair_core::{
    geom::GridSize,
    ids::{ClientId, PaneId},
    proto::ServerMsg,
    time::Millis,
};
use flair_term::pty::SpawnSpec;
use std::path::PathBuf;

use crate::event::Timer;

#[derive(Debug)]
pub enum Effect {
    Spawn(PaneId, SpawnSpec),
    SpawnSplit(PaneId, PaneId, SpawnSpec),
    WritePty(PaneId, Vec<u8>),
    ResizePty(PaneId, GridSize),
    KillPty(PaneId),
    Send(ClientId, ServerMsg),
    SetTimer(Timer, Millis),
    ReadConfig(PathBuf),
    ReadGit {
        cwd: PathBuf,
        changes: bool,
        panes: Vec<PaneId>,
    },
    ReadContexts,
    Shutdown,
}
