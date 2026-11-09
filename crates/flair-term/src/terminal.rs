use crate::{listener::Collector, osc::OscSniffer};
use alacritty_terminal::{
    Term,
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{Config as TermConfig, TermMode},
    vte::ansi::Processor,
};
use flair_core::{
    geom::{CellPoint, GridSize},
    proto::PaneUpdate,
    theme::Theme,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct Terminal {
    pub(crate) term: Term<Collector>,
    processor: Processor,
    osc: OscSniffer,
    clear: u8,
    events: Rc<RefCell<Vec<TermEvent>>>,
    palette: Rc<RefCell<Theme>>,
    title: Option<String>,
    cwd: Option<PathBuf>,
    pub(crate) size: GridSize,
    pub(crate) previous: Option<PaneUpdate>,
    pub(crate) force_full: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermEvent {
    Title(String),
    Bell,
    PtyWrite(Vec<u8>),
    Cwd(PathBuf),
    Mark(MarkKind, Option<i32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    PromptStart,
    PromptEnd,
    CommandStart,
    CommandEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes {
    pub app_cursor: bool,
    pub app_keypad: bool,
    pub bracketed_paste: bool,
    pub focus_events: bool,
    pub mouse: MouseMode,
    pub sgr_mouse: bool,
    pub alt_screen: bool,
    pub kitty_flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseMode {
    #[default]
    Off,
    Click,
    Drag,
    Motion,
}

impl Terminal {
    pub fn new(size: GridSize, scrollback: usize) -> Terminal {
        let size = GridSize {
            cols: size.cols.max(1),
            rows: size.rows.max(1),
        };
        let events = Rc::new(RefCell::new(Vec::new()));
        let palette = Rc::new(RefCell::new(flair_core::config::Config::default().theme()));
        let collector = Collector {
            events: events.clone(),
            palette: palette.clone(),
        };
        let config = TermConfig {
            scrolling_history: scrollback,
            kitty_keyboard: true,
            ..TermConfig::default()
        };
        Terminal {
            term: Term::new(config, &Size(size), collector),
            processor: Processor::new(),
            osc: OscSniffer::default(),
            clear: 0,
            events,
            palette,
            title: None,
            cwd: None,
            size,
            previous: None,
            force_full: true,
        }
    }
    pub fn advance(&mut self, bytes: &[u8]) {
        let old_alt = self.modes().alt_screen;
        self.processor.advance(&mut self.term, bytes);
        self.osc.advance(bytes);
        self.events.borrow_mut().extend(self.osc.take());
        for event in self.events.borrow().iter() {
            match event {
                TermEvent::Title(title) => self.title = Some(title.clone()),
                TermEvent::Cwd(cwd) => self.cwd = Some(cwd.clone()),
                TermEvent::Bell | TermEvent::PtyWrite(_) | TermEvent::Mark(_, _) => (),
            }
        }
        let alt_changed = old_alt != self.modes().alt_screen;
        if sniff_clear(&mut self.clear, bytes) || alt_changed {
            self.exit_copy_mode();
        }
        self.force_full |= alt_changed;
    }
    pub fn copy_mode(&self) -> bool {
        self.term.mode().contains(TermMode::VI)
    }
    pub fn enter_copy_mode(&mut self) {
        if !self.copy_mode() {
            self.term.toggle_vi_mode();
            self.force_full = true;
        }
    }
    pub fn exit_copy_mode(&mut self) {
        if self.copy_mode() {
            self.term.toggle_vi_mode();
            self.clear_selection();
            self.term.scroll_display(Scroll::Bottom);
            self.force_full = true;
        }
    }
    pub fn resize(&mut self, size: GridSize) {
        self.exit_copy_mode();
        self.clear_selection();
        self.size = GridSize {
            cols: size.cols.max(1),
            rows: size.rows.max(1),
        };
        self.term.resize(Size(self.size));
        self.force_full = true;
    }
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
        self.force_full = true;
    }
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }
    pub fn scroll_to_bottom(&mut self) {
        if self.term.grid().display_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);
            self.force_full = true;
        }
    }
    pub fn select_start(&mut self, point: CellPoint, clicks: u8) {
        let point = self.selection_point(point);
        let ty = match clicks {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            0 | 1 => SelectionType::Simple,
        };
        self.term.selection = Some(Selection::new(ty, point, Side::Left));
    }
    pub fn select_update(&mut self, point: CellPoint) {
        let point = self.selection_point(point);
        if let Some(selection) = &mut self.term.selection {
            selection.update(point, Side::Right);
            selection.include_all();
        }
    }
    pub fn selection_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }
    fn selection_point(&self, point: CellPoint) -> Point {
        Point::new(
            Line(
                i32::from(point.row.min(self.size.rows - 1))
                    - self.term.grid().display_offset() as i32,
            ),
            Column(usize::from(point.col.min(self.size.cols - 1))),
        )
    }
    pub fn set_palette(&mut self, theme: &Theme) {
        *self.palette.borrow_mut() = theme.clone();
    }
    pub fn modes(&self) -> Modes {
        let m = self.term.mode();
        Modes {
            app_cursor: m.contains(TermMode::APP_CURSOR),
            app_keypad: m.contains(TermMode::APP_KEYPAD),
            bracketed_paste: m.contains(TermMode::BRACKETED_PASTE),
            focus_events: m.contains(TermMode::FOCUS_IN_OUT),
            mouse: if m.contains(TermMode::MOUSE_MOTION) {
                MouseMode::Motion
            } else if m.contains(TermMode::MOUSE_DRAG) {
                MouseMode::Drag
            } else if m.contains(TermMode::MOUSE_REPORT_CLICK) {
                MouseMode::Click
            } else {
                MouseMode::Off
            },
            sgr_mouse: m.contains(TermMode::SGR_MOUSE),
            alt_screen: m.contains(TermMode::ALT_SCREEN),
            kitty_flags: ((m.bits() & TermMode::KITTY_KEYBOARD_PROTOCOL.bits()) >> 18) as u8,
        }
    }
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }
    pub fn take_events(&mut self) -> Vec<TermEvent> {
        std::mem::take(&mut self.events.borrow_mut())
    }
}
fn sniff_clear(state: &mut u8, bytes: &[u8]) -> bool {
    let mut seen = false;
    for &b in bytes {
        *state = match (*state, b) {
            (_, 27) => 1,
            (1, b'c') | (3, b'J') => {
                seen = true;
                0
            }
            (1, b'[') => 2,
            (2, b'2' | b'3') => 3,
            _ => 0,
        };
    }
    seen
}
struct Size(GridSize);
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.0.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.0.rows as usize
    }
    fn columns(&self) -> usize {
        self.0.cols as usize
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
