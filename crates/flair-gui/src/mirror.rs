use flair_core::{
    geom::GridSize,
    ids::PaneId,
    isles::PromptInfo,
    proto::{Cell, Cursor, PaneFlags, PaneUpdate, SelectionSpan, WindowLayout},
};
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub struct Mirror {
    panes: HashMap<PaneId, PaneMirror>,
    prompts: BTreeMap<PaneId, PromptInfo>,
}

pub struct PaneMirror {
    pub size: GridSize,
    pub rows: Vec<Vec<Cell>>,
    pub cursor: Cursor,
    pub flags: PaneFlags,
    pub selection: Vec<SelectionSpan>,
}

impl Mirror {
    pub fn retain_layout(&mut self, layout: &WindowLayout) {
        let present = |id: &PaneId| layout.panes.iter().any(|placement| placement.pane == *id);
        self.panes.retain(|id, _| present(id));
        self.prompts.retain(|id, _| present(id));
    }

    pub fn insert_prompt(&mut self, pane: PaneId, info: PromptInfo) {
        self.prompts.insert(pane, info);
    }

    pub fn prompts(&self) -> &BTreeMap<PaneId, PromptInfo> {
        &self.prompts
    }

    pub fn apply(&mut self, updates: &[PaneUpdate]) -> bool {
        let mut changed = false;
        for update in updates {
            if update.size.cols == 0 || update.size.rows == 0 {
                continue;
            }
            if update.full {
                self.panes.insert(
                    update.pane,
                    PaneMirror {
                        size: update.size,
                        rows: vec![
                            vec![Cell::default(); usize::from(update.size.cols)];
                            usize::from(update.size.rows)
                        ],
                        cursor: update.cursor,
                        flags: update.flags,
                        selection: update.selection.clone(),
                    },
                );
            }
            let Some(pane) = self.panes.get_mut(&update.pane) else {
                continue;
            };
            if pane.size != update.size {
                continue;
            }
            for (index, row) in &update.rows {
                if let Some(target) = pane.rows.get_mut(usize::from(*index)) {
                    target.fill(Cell::default());
                    for (dest, cell) in target.iter_mut().zip(row) {
                        *dest = cell.clone();
                    }
                }
            }
            pane.cursor = update.cursor;
            pane.flags = update.flags;
            pane.selection = update.selection.clone();
            changed = true;
        }
        changed
    }

    pub fn pane(&self, id: PaneId) -> Option<&PaneMirror> {
        self.panes.get(&id)
    }
}

#[cfg(test)]
#[path = "mirror_tests.rs"]
mod tests;
