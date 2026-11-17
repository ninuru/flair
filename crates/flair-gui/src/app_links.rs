use std::process::{Command, Stdio};

use flair_core::geom::CellRect;

use super::*;
use crate::{
    cellgrid::CellGrid,
    links::{self, Link},
    mouse_route::pane_cell,
};

impl App {
    fn hovered_link(&self) -> Option<(CellRect, Link)> {
        if !self.mods.control_key()
            || !matches!(self.capture, Capture::Idle | Capture::Overlay)
            || self.overlays.is_open()
            || self.in_chrome()
            || chrome_layout::hit_test(&self.hits, (self.pointer.x, self.pointer.y)).is_some()
        {
            return None;
        }
        let point = self.cell()?;
        let (pane, local, rect) = pane_cell(self.layout.as_ref(), self.pane, self.size(), point)?;
        let link = links::link_at(&self.mirror.pane(pane)?.rows, local)?;
        Some((rect, link))
    }

    pub(super) fn update_link_hover(&mut self) {
        let link = self.hovered_link().map(|(_, link)| link);
        if link != self.hover_link {
            self.hover_link = link;
            self.request_redraw();
        }
        self.update_cursor();
    }

    pub(super) fn underline_link(&mut self, grid: &mut CellGrid) {
        let hovered = self.hovered_link();
        if let Some((rect, link)) = &hovered {
            links::underline(grid, link, *rect);
        }
        self.hover_link = hovered.map(|(_, link)| link);
        self.update_cursor();
    }

    pub(super) fn open_link(&mut self) -> bool {
        if self.capture != Capture::Idle {
            return false;
        }
        let Some((_, link)) = self.hovered_link() else {
            return false;
        };
        let child = Command::new("xdg-open")
            .arg(&link.url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(mut child) => {
                if let Err(error) =
                    thread::Builder::new()
                        .name("flair-xdg-open".into())
                        .spawn(move || match child.wait() {
                            Ok(status) if !status.success() => {
                                log::warn!("xdg-open failed: {status}");
                            }
                            Ok(_) => {}
                            Err(error) => log::warn!("xdg-open did not finish: {error}"),
                        })
                {
                    log::warn!("could not wait for xdg-open: {error}");
                }
            }
            Err(error) => {
                log::warn!("could not run xdg-open: {error}");
                self.overlays
                    .set_notice(Notice::error(format!("Could not open link: {error}")));
            }
        }
        self.request_redraw();
        true
    }
}
