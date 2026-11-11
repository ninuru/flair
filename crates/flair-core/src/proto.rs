use std::{
    ops::{BitOr, BitOrAssign},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::{
    command::Command,
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::{ClientId, PaneId, WindowId},
    isles::PromptInfo,
    key::InputEvent,
    status::StatusLine,
    theme::Theme,
};

pub const PROTOCOL: u32 = 11;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello {
        protocol: u32,
        kind: ClientKind,
        size: GridSize,
        palette: Theme,
        env: Vec<(String, String)>,
        target: Target,
    },
    Resize(GridSize),
    Input(InputEvent),
    Focus(bool),
    FrameAck {
        seq: u64,
    },
    Command(Command),
    Palette(Theme),
    Detach,
    Control(Control),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        client: ClientId,
    },
    Frame {
        seq: u64,
        panes: Vec<PaneUpdate>,
    },
    Layout(WindowLayout),
    Status(StatusLine),
    Title(String),
    Bell,
    SetClipboard {
        text: String,
        primary: bool,
    },
    Sessionizer {
        sessions: Vec<SessionInfo>,
        current: String,
        last: Option<Target>,
    },
    Sessions(Vec<SessionInfo>),
    ChooseWindow,
    ChooseTheme,
    Settings,
    PasteClipboard,
    PromptRename {
        current: String,
    },
    PromptSearch {
        forward: bool,
    },
    ReloadConfig {
        toml: String,
    },
    Done,
    Error(String),
    Exit(ExitReason),
    ChooseSession {
        sessions: Vec<SessionInfo>,
        current: String,
    },
    PromptInfo {
        pane: PaneId,
        info: PromptInfo,
    },
    PromptHistory {
        pane: PaneId,
        prompts: Vec<PromptSnapshot>,
    },
    PaneProcesses(Vec<u32>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSnapshot {
    pub row: u16,
    pub id: u64,
    pub info: PromptInfo,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneUpdate {
    pub pane: PaneId,
    pub size: GridSize,
    pub full: bool,
    pub rows: Vec<(u16, Vec<Cell>)>,
    pub cursor: Cursor,
    pub flags: PaneFlags,
    pub selection: Vec<SelectionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
    pub extra: Option<String>,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            c: ' ',
            fg: Color::Foreground,
            bg: Color::Background,
            flags: CellFlags::empty(),
            extra: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellFlags(u16);

impl CellFlags {
    pub const BOLD: CellFlags = CellFlags(1);
    pub const DIM: CellFlags = CellFlags(1 << 1);
    pub const ITALIC: CellFlags = CellFlags(1 << 2);
    pub const UNDERLINE: CellFlags = CellFlags(1 << 3);
    pub const STRIKE: CellFlags = CellFlags(1 << 4);
    pub const INVERSE: CellFlags = CellFlags(1 << 5);
    pub const HIDDEN: CellFlags = CellFlags(1 << 6);
    pub const WIDE: CellFlags = CellFlags(1 << 7);
    pub const WIDE_SPACER: CellFlags = CellFlags(1 << 8);

    pub const fn empty() -> CellFlags {
        CellFlags(0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: CellFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: CellFlags) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: CellFlags) {
        self.0 &= !other.0;
    }
}

impl BitOr for CellFlags {
    type Output = CellFlags;

    fn bitor(self, rhs: CellFlags) -> CellFlags {
        CellFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for CellFlags {
    fn bitor_assign(&mut self, rhs: CellFlags) {
        self.insert(rhs);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    Foreground,
    Background,
    Cursor,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub point: CellPoint,
    pub shape: CursorShape,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorShape {
    Block,
    Underline,
    Beam,
    HollowBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PaneFlags {
    pub prompt_row: Option<u16>,
    pub alt_screen: bool,
    pub mouse_reporting: bool,
    pub display_offset: u32,
    pub copy_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientKind {
    Gui,
    Headless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Main,
    New,
    Name(String),
    Path(PathBuf),
    Create { name: Option<String>, root: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    Detached,
    SessionEnded,
    ServerExited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    List,
    KillServer,
    Create {
        target: Target,
        env: Vec<(String, String)>,
    },
    Switch {
        target: Target,
        pane: Option<PaneId>,
        env: Vec<(String, String)>,
    },
    PaneProcesses {
        pane: Option<PaneId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub name: String,
    pub root: PathBuf,
    pub windows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionSpan {
    pub row: u16,
    pub start: u16,
    pub end: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLayout {
    pub window: WindowId,
    pub area: CellRect,
    pub panes: Vec<PanePlacement>,
    pub separators: Vec<Separator>,
    pub active: PaneId,
    pub zoomed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanePlacement {
    pub pane: PaneId,
    pub rect: CellRect,
    pub band: u16,
}

impl PanePlacement {
    pub fn outer(self) -> CellRect {
        CellRect {
            origin: CellPoint {
                row: self.rect.origin.row.saturating_sub(self.band),
                ..self.rect.origin
            },
            size: GridSize {
                rows: self.rect.size.rows.saturating_add(self.band),
                ..self.rect.size
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Separator {
    pub axis: Axis,
    pub rect: CellRect,
}
