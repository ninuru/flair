use flair_core::{
    config::{Config, EXPLORER_WIDTH, Side, TABS_WIDTH, TabsPosition},
    ids::WindowId,
};

use crate::{
    chrome_layout::{Dock, Frame, PanelView},
    panel_layout::tab_rows,
    ui::{DRAG_START, px},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pages {
    pub tabs: usize,
    pub explorer: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    Tab {
        window: WindowId,
        from: f32,
        moved: bool,
    },
    Edge {
        side: Side,
        from: f32,
        width: f32,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Panels {
    pub flipped: Option<TabsPosition>,
    pub shown: PanelView,
    pub tabs_scroll: usize,
    pub drag: Option<Drag>,
    pub live_width: Option<(PanelView, f32)>,
    pages: Pages,
}

impl Panels {
    pub fn dock(&self, config: &Config, zen: bool) -> Dock {
        let width = |view, configured: u16| match self.live_width {
            Some((live, width)) if live == view => width,
            Some(_) | None => f32::from(configured),
        };
        let flipped = self.flipped == Some(config.tabs.position);
        let side = match (config.tabs.position, flipped) {
            (TabsPosition::Top, false) | (TabsPosition::Left | TabsPosition::Right, true) => None,
            (TabsPosition::Top | TabsPosition::Left, _) => Some(Side::Left),
            (TabsPosition::Right, false) => Some(Side::Right),
        };
        Dock {
            tabs: side
                .filter(|_| !zen)
                .map(|side| (side, width(PanelView::Tabs, config.tabs.width))),
            explorer: None,
            shown: self.shown,
        }
    }

    pub fn toggle_tabs(&mut self, position: TabsPosition) {
        self.flipped = match self.flipped == Some(position) {
            true => None,
            false => Some(position),
        };
        self.drag = None;
        self.live_width = None;
    }

    pub fn place(&mut self, pages: Pages) {
        self.pages = pages;
    }

    pub fn reveal_tab(&mut self, active: usize) {
        let page = self.pages.tabs;
        if page == 0 {
            return;
        }
        if active < self.tabs_scroll {
            self.tabs_scroll = active;
        } else if active >= self.tabs_scroll + page {
            self.tabs_scroll = active + 1 - page;
        }
    }

    pub fn scroll_tabs(&mut self, rows: i32, count: usize) {
        let last = (count + 1).saturating_sub(self.pages.tabs);
        let next = self.tabs_scroll.min(last) as i64 + i64::from(rows);
        self.tabs_scroll = next.clamp(0, last as i64) as usize;
    }

    pub fn drop_index(&self, frame: &Frame, y: f32, count: usize, scale: f32) -> u16 {
        let Some(slot) = frame.slots().find(|slot| slot.view == PanelView::Tabs) else {
            return 1;
        };
        let rows = tab_rows(slot, scale);
        let first = self.tabs_scroll.min((count + 1).saturating_sub(rows.page));
        let row = ((y - rows.top) / rows.pitch).floor().max(0.0) as usize + first;
        u16::try_from(row.min(count.saturating_sub(1)) + 1).unwrap_or(u16::MAX)
    }

    pub fn drag_tab(&mut self, y: f32, scale: f32) -> bool {
        match &mut self.drag {
            Some(Drag::Tab { from, moved, .. }) if !*moved => {
                *moved = (y - *from).abs() > px(DRAG_START, scale);
                *moved
            }
            Some(Drag::Tab { .. } | Drag::Edge { .. }) | None => false,
        }
    }

    pub fn drag_edge(&mut self, view: PanelView, x: f32, limit: f32, scale: f32) -> bool {
        let Some(Drag::Edge { side, from, width }) = self.drag else {
            return false;
        };
        let (min, max) = bounds(view);
        if limit < min {
            return self.live_width.take().is_some();
        }
        let delta = (x - from) / scale;
        let wanted = match side {
            Side::Left => width + delta,
            Side::Right => width - delta,
        };
        let next = wanted.round().min(max.min(limit)).max(min);
        let changed = self.live_width != Some((view, next));
        self.live_width = Some((view, next));
        changed
    }
}

pub fn bounds(view: PanelView) -> (f32, f32) {
    let (min, max) = match view {
        PanelView::Tabs => TABS_WIDTH,
        PanelView::Explorer => EXPLORER_WIDTH,
    };
    (f32::from(min), f32::from(max))
}

#[cfg(test)]
#[path = "panels_tests.rs"]
mod tests;
