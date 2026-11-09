use serde::{Deserialize, Serialize};

use crate::ids::WindowId;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatusLine {
    pub session: String,
    pub hidden: u16,
    pub running: bool,
    pub finished: bool,
    pub windows: Vec<StatusWindow>,
    pub pending: Option<String>,
    pub copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusWindow {
    pub id: WindowId,
    pub index: u16,
    pub name: String,
    pub active: bool,
    pub last: bool,
}
