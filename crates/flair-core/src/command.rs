use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    ScrollLines(i32),
    ScrollPages(i32),
    ScrollToBottom,
}
