use flair_core::status::ActivePane;

use super::*;
use crate::{
    chrome_layout::PanelView,
    explorer::{ExplorerOutcome, Listing},
    explorer_fs::FsWorker,
    panels::ExplorerAct,
};

impl App {
    pub(super) fn explorer_command(&mut self) {
        let dirs = self.panels.explorer_command();
        self.explorer_toggled(dirs);
    }

    pub(super) fn toggle_explorer(&mut self) {
        let dirs = self.panels.toggle_explorer();
        self.explorer_toggled(dirs);
    }

    fn explorer_toggled(&mut self, dirs: Vec<PathBuf>) {
        self.load(dirs);
        self.send(ClientMsg::Resize(self.size()));
        self.request_redraw();
    }

    pub(super) fn follow_pane(&mut self, pane: Option<ActivePane>) {
        let dirs = self.panels.follow(pane);
        self.load(dirs);
    }

    pub(super) fn listing(&mut self, listing: Listing) {
        if self.panels.explorer.insert(listing) {
            self.request_redraw();
        }
    }

    pub(super) fn explorer_key(&mut self, key: &KeyInput) -> bool {
        if !self.explorer_visible() {
            return false;
        }
        let config = self
            .preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config);
        match self.panels.key(key, &config.keys, Instant::now()) {
            Some(outcome) => {
                self.explorer_outcome(outcome);
                true
            }
            None => false,
        }
    }

    pub(super) fn explorer_text(&mut self, text: &str) -> bool {
        if !self.explorer_visible() {
            return false;
        }
        match self.panels.text(text) {
            Some(outcome) => {
                self.explorer_outcome(outcome);
                true
            }
            None => false,
        }
    }

    pub(super) fn explorer_outcome(&mut self, outcome: ExplorerOutcome) {
        match self.panels.act(outcome) {
            ExplorerAct::Ignored => {}
            ExplorerAct::Redraw => self.request_redraw(),
            ExplorerAct::Load(dirs) => {
                self.load(dirs);
                self.request_redraw();
            }
            ExplorerAct::Send(message) => {
                self.send(message);
                self.request_redraw();
            }
            ExplorerAct::Copy(path) => {
                if let Some(surface) = &self.surface {
                    surface
                        .clipboard
                        .store(path.to_string_lossy().into_owned(), false);
                }
                self.overlays.set_notice(Notice::info("copied path"));
                self.request_redraw();
            }
        }
    }

    pub(super) fn blur_explorer(&mut self) {
        if self.panels.focused {
            self.panels.focused = false;
            self.request_redraw();
        }
    }

    fn explorer_visible(&self) -> bool {
        self.frame()
            .is_some_and(|frame| frame.slots().any(|slot| slot.view == PanelView::Explorer))
    }

    fn load(&mut self, dirs: Vec<PathBuf>) {
        if dirs.is_empty() {
            return;
        }
        if self.fs.is_none() {
            let proxy = self.proxy.clone();
            match FsWorker::spawn(move |listing| {
                let _ = proxy.send_event(UserEvent::Listing(listing));
            }) {
                Ok(worker) => self.fs = Some(worker),
                Err(error) => {
                    log::warn!("could not start the explorer reader: {error}");
                    self.overlays
                        .set_notice(Notice::error(format!("explorer not started: {error}")));
                    return;
                }
            }
        }
        if let Some(worker) = &self.fs {
            for dir in dirs {
                worker.request(dir);
            }
        }
    }
}
