use flair_core::{
    agent::AgentRecord,
    command::JumpKind,
    geom::GridSize,
    isles::{LastCommand, ShellFacts},
    proto::PaneUpdate,
    time::Millis,
};
use flair_term::Terminal;
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) struct Pane {
    pub terminal: Terminal,
    pub size: GridSize,
    pub cwd: PathBuf,
    pub dirty: bool,
    pub cached: Option<PaneUpdate>,
    pub exited: bool,
    pub started: Option<Millis>,
    pub last: Option<LastCommand>,
    pub shell: Option<ShellFacts>,
    pub wheel_copy: bool,
    pub copy_keys: CopyKeys,
    pub process: Option<(String, String)>,
    pub probed: Option<Millis>,
    pub probe_due: Option<Millis>,
    pub at_prompt: bool,
    pub agents: BTreeMap<String, AgentRecord>,
    pub agent_program: Option<String>,
    pub unseen: bool,
    pub notified: Option<Millis>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CopyKeys {
    pub count: Option<u16>,
    pub awaiting: Option<Awaiting>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Awaiting {
    Jump(JumpKind),
    G,
}
