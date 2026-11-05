use std::path::{Path, PathBuf};

use flair_core::{geom::GridSize, theme::Theme};

pub struct Terminal {}

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
        todo!()
    }

    pub fn advance(&mut self, bytes: &[u8]) {
        todo!()
    }

    pub fn resize(&mut self, size: GridSize) {
        todo!()
    }

    pub fn scroll(&mut self, lines: i32) {
        todo!()
    }

    pub fn scroll_to_bottom(&mut self) {
        todo!()
    }

    pub fn set_palette(&mut self, theme: &Theme) {
        todo!()
    }

    pub fn modes(&self) -> Modes {
        todo!()
    }

    pub fn title(&self) -> Option<&str> {
        todo!()
    }

    pub fn cwd(&self) -> Option<&Path> {
        todo!()
    }

    pub fn take_events(&mut self) -> Vec<TermEvent> {
        todo!()
    }
}
