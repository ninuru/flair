use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::time::Millis;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    User,
    Host,
    Cwd,
    GitBranch,
    GitAheadBehind,
    GitChanges,
    Venv,
    Docker,
    Kube,
    Exit,
    Time,
}

impl Chip {
    pub const ALL: [Chip; 11] = [
        Chip::User,
        Chip::Host,
        Chip::Cwd,
        Chip::GitBranch,
        Chip::GitAheadBehind,
        Chip::GitChanges,
        Chip::Venv,
        Chip::Docker,
        Chip::Kube,
        Chip::Exit,
        Chip::Time,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PromptInfo {
    pub cwd: Option<PathBuf>,
    pub git: Option<GitInfo>,
    pub last: Option<LastCommand>,
    pub shell: Option<ShellFacts>,
    pub docker: Option<String>,
    pub kube: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub head: Head,
    pub upstream: Option<AheadBehind>,
    pub changes: Option<Changes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Head {
    Branch(String),
    Detached(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub files: u32,
    pub added: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastCommand {
    pub exit: Option<i32>,
    pub duration: Millis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellFacts {
    pub user: String,
    pub host: String,
    pub venv: Option<String>,
}

#[cfg(test)]
#[path = "isles_tests.rs"]
mod tests;
