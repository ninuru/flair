use std::{path::PathBuf, time::Instant};

use flair_core::{key::KeyInput, proto::ClientMsg, status::ActivePane};
use flair_gui::{
    chrome_layout::PanelView,
    explorer::{ExplorerOutcome, RowKind},
    explorer_fs::read_listing,
    notice::Notice,
    panels::ExplorerAct,
};

use super::Capture;

impl Capture {
    pub(super) fn explorer_command(&mut self) -> Result<Option<ClientMsg>, String> {
        let dirs = self.panels.explorer_command();
        self.load(dirs);
        self.refit()
    }

    pub(super) fn follow(&mut self, pane: Option<ActivePane>) {
        let dirs = self.panels.follow(pane);
        self.load(dirs);
    }

    pub(super) fn explorer_key(&mut self, key: &KeyInput) -> Option<Vec<ClientMsg>> {
        if !self.explorer_visible() {
            return None;
        }
        let config = self
            .preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config);
        let pending = self.status.pending.is_some();
        let outcome = self
            .panels
            .key(key, &config.keys, pending, Instant::now())?;
        Some(self.explorer_outcome(outcome))
    }

    pub(super) fn explorer_text(&mut self, text: &str) -> Option<Vec<ClientMsg>> {
        if !self.explorer_visible() {
            return None;
        }
        let outcome = self.panels.text(text)?;
        Some(self.explorer_outcome(outcome))
    }

    pub(super) fn explorer_lines(&self) -> Option<String> {
        if !self.explorer_visible() {
            return None;
        }
        let explorer = &self.panels.explorer;
        let root = explorer.root().map_or_else(
            || "no directory".to_owned(),
            |root| root.display().to_string(),
        );
        let mut lines = vec![format!("[explorer {root}]")];
        lines.extend(explorer.filter().map(|filter| format!("/{filter}")));
        for (index, row) in explorer.rows().iter().enumerate() {
            let mark = match explorer.selected() == Some(index) {
                true => "*",
                false => " ",
            };
            let label = match &row.kind {
                RowKind::Dir { open: true } => format!("v {}", row.name),
                RowKind::Dir { open: false } => format!("> {}", row.name),
                RowKind::File => row.name.clone(),
                RowKind::More(count) => format!("+{count} more"),
                RowKind::Loading => "...".to_owned(),
                RowKind::Error(error) => format!("! {error}"),
            };
            let indent = "  ".repeat(usize::from(row.depth));
            lines.push(format!("{mark}{indent}{label}"));
        }
        Some(lines.join("\n"))
    }

    pub(super) fn sync_hidden(&mut self) {
        let show = self.effective().explorer.show_hidden;
        self.panels.show_hidden(show);
        let editor = self.effective().explorer.editor.clone();
        self.panels.set_editor(editor);
    }

    fn explorer_outcome(&mut self, outcome: ExplorerOutcome) -> Vec<ClientMsg> {
        match self.panels.act(outcome) {
            ExplorerAct::Ignored | ExplorerAct::Redraw => Vec::new(),
            ExplorerAct::Load(dirs) => {
                self.load(dirs);
                Vec::new()
            }
            ExplorerAct::Send(message) => vec![message],
            ExplorerAct::Copy(_) => {
                self.overlays.set_notice(Notice::info("copied path"));
                Vec::new()
            }
            ExplorerAct::Notice(notice) => {
                self.overlays.set_notice(notice);
                Vec::new()
            }
        }
    }

    fn explorer_visible(&self) -> bool {
        match &self.pixels {
            Some(pixels) => pixels
                .frame(self.effective(), self.zen, &self.panels)
                .slots()
                .any(|slot| slot.view == PanelView::Explorer),
            None => self.panels.open && !self.zen && self.panels.shown != PanelView::Agents,
        }
    }

    pub(super) fn load(&mut self, dirs: Vec<PathBuf>) {
        self.sync_hidden();
        for dir in dirs {
            self.panels.explorer.insert(read_listing(&dir));
        }
    }
}
