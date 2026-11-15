use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{
    agent::{AgentKind, AgentState},
    ids::{PaneId, WindowId},
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatusLine {
    pub session: String,
    pub hidden: u16,
    pub running: bool,
    pub finished: bool,
    pub windows: Vec<StatusWindow>,
    pub pending: Option<String>,
    pub copy: bool,
    pub pane: Option<ActivePane>,
    pub agents: Vec<AgentRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivePane {
    pub pane: PaneId,
    pub cwd: Option<PathBuf>,
    pub at_prompt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusWindow {
    pub id: WindowId,
    pub index: u16,
    pub name: String,
    pub program: String,
    pub active: bool,
    pub last: bool,
    pub activity: bool,
    pub bell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRow {
    pub session: String,
    pub window: WindowId,
    pub pane: PaneId,
    pub app: String,
    pub state: AgentState,
    pub kind: Option<AgentKind>,
    pub title: String,
    pub unseen: bool,
}
