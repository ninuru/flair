use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentKind {
    Permission,
    Question,
    Auth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub state: AgentState,
    pub kind: Option<AgentKind>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentReport {
    Set { id: String, record: AgentRecord },
    Clear(Option<String>),
}

impl AgentState {
    pub fn rank(self) -> u8 {
        match self {
            AgentState::Blocked => 4,
            AgentState::Error => 3,
            AgentState::Working => 2,
            AgentState::Done => 1,
            AgentState::Idle => 0,
        }
    }

    pub fn label(self, kind: Option<AgentKind>) -> &'static str {
        match self {
            AgentState::Blocked => match kind {
                Some(AgentKind::Permission) => "Needs approval",
                Some(AgentKind::Auth) => "Needs login",
                Some(AgentKind::Question) | None => "Needs input",
            },
            AgentState::Working => "Working",
            AgentState::Done => "Done",
            AgentState::Error => "Failed",
            AgentState::Idle => "Idle",
        }
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
