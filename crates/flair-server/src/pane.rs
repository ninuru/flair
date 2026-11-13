use flair_core::{
    geom::GridSize,
    isles::{LastCommand, ShellFacts},
    proto::PaneUpdate,
    time::Millis,
};
use flair_term::Terminal;
use std::path::PathBuf;

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
    pub process: Option<(String, String)>,
    pub probed: Option<Millis>,
    pub probe_due: Option<Millis>,
}
