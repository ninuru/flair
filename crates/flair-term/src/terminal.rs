use crate::{listener::Collector, osc::OscSniffer};
use alacritty_terminal::{
    Term,
    grid::{Dimensions, Scroll},
    term::{Config as TermConfig, TermMode},
    vte::ansi::Processor,
};
use flair_core::{geom::GridSize, proto::PaneUpdate, theme::Theme};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct Terminal {
    pub(crate) term: Term<Collector>,
    processor: Processor,
    osc: OscSniffer,
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
        self.force_full |= old_alt != self.modes().alt_screen;
    }
    pub fn resize(&mut self, size: GridSize) {
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
    pub fn scroll_to_bottom(&mut self) {
        if self.term.grid().display_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);
            self.force_full = true;
        }
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
