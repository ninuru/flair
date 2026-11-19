use crate::{
    geom::{Axis, Direction},
    ids::{PaneId, SessionId, WindowId},
    proto::Target,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    ScrollLines(i32),
    ScrollPages(i32),
    ScrollToBottom,
    Switch(Target),
    LastSession,
    Sessionizer,
    CopySelection,
    PasteClipboard,
    Detach,
    NewWindow,
    NextWindow,
    PreviousWindow,
    LastWindow,
    SelectWindow(WindowRef),
    ChooseWindow,
    RenameWindow(Option<String>),
    CloseWindow,
    Split(Axis),
    SelectPane(PaneRef),
    ClosePane,
    ToggleZoom,
    ToggleZen,
    ToggleTabs,
    MoveWindow { window: WindowId, index: u16 },
    KillWindow(WindowId),
    KillSession(SessionId),
    Explorer,
    Agents,
    SplitRun { argv: Vec<String>, cwd: PathBuf },
    ReloadConfig,
    ChooseSession,
    ChooseTheme,
    Settings,
    CopyMode,
    Copy(CopyCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyCommand {
    Cancel,
    Move(CopyMotion),
    Select(SelectionKind),
    OtherEnd,
    SearchPrompt { forward: bool },
    Search { pattern: String, forward: bool },
    SearchAgain,
    SearchReverse,
    JumpPrompt(JumpKind),
    Jump { kind: JumpKind, ch: char },
    JumpAgain,
    JumpReverse,
    GPrefix,
    Yank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JumpKind {
    Forward,
    Backward,
    ToForward,
    ToBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyMotion {
    Left,
    Down,
    Up,
    Right,
    NextWord,
    PreviousWord,
    NextWordEnd,
    PreviousWordEnd,
    NextSpace,
    PreviousSpace,
    NextSpaceEnd,
    PreviousSpaceEnd,
    StartOfLine,
    BackToIndentation,
    EndOfLine,
    HistoryTop,
    HistoryBottom,
    HalfPageUp,
    HalfPageDown,
    TopLine,
    MiddleLine,
    BottomLine,
    ParagraphUp,
    ParagraphDown,
    Bracket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionKind {
    Simple,
    Lines,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowRef {
    Index(u16),
    Id(WindowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneRef {
    Direction(Direction),
    Id(PaneId),
}
