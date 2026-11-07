use serde::{Deserialize, Serialize};

use crate::ids::WindowId;

pub const STATUS_ROWS: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatusLine {
    pub session: String,
    pub windows: Vec<StatusWindow>,
    pub pending: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusWindow {
    pub id: WindowId,
    pub index: u16,
    pub name: String,
    pub active: bool,
    pub last: bool,
}
