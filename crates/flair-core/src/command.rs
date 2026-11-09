use crate::{
    geom::{Axis, Direction},
    ids::{PaneId, WindowId},
    proto::Target,
};
use serde::{Deserialize, Serialize};

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
    ReloadConfig,
    ChooseSession,
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
    Yank,
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
    NextSpace,
    PreviousSpace,
    NextSpaceEnd,
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
