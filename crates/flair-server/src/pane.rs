use flair_core::{geom::GridSize, proto::PaneUpdate};
use flair_term::Terminal;
use std::path::PathBuf;

pub(crate) struct Pane {
    pub terminal: Terminal,
    pub size: GridSize,
    pub cwd: PathBuf,
    pub dirty: bool,
    pub cached: Option<PaneUpdate>,
    pub exited: bool,
    pub wheel_copy: bool,
}
