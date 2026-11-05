use crate::proto::Target;
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
}
