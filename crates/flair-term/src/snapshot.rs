use flair_core::{ids::PaneId, proto::PaneUpdate};

use crate::terminal::Terminal;

impl Terminal {
    pub fn snapshot(&mut self, pane: PaneId, full: bool) -> Option<PaneUpdate> {
        todo!()
    }
}
