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
