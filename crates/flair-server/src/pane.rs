use flair_core::{geom::GridSize, proto::PaneUpdate};
use flair_term::Terminal;

pub(crate) struct Pane {
    pub terminal: Terminal,
    pub size: GridSize,
    pub dirty: bool,
    pub cached: Option<PaneUpdate>,
    pub exited: bool,
}
