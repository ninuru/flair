use flair_core::geom::GridSize;
use flair_term::Terminal;

pub(crate) struct Pane {
    pub terminal: Terminal,
    pub size: GridSize,
    pub dirty: bool,
    pub full: bool,
}
