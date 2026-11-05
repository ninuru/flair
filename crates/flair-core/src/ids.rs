use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WindowId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClientId(pub u32);

#[derive(Debug, Default)]
pub struct IdGen {
    session: u32,
    window: u32,
    pane: u32,
    client: u32,
}

impl IdGen {
    pub fn next_session(&mut self) -> SessionId {
        SessionId(bump(&mut self.session))
    }

    pub fn next_window(&mut self) -> WindowId {
        WindowId(bump(&mut self.window))
    }

    pub fn next_pane(&mut self) -> PaneId {
        PaneId(bump(&mut self.pane))
    }

    pub fn next_client(&mut self) -> ClientId {
        ClientId(bump(&mut self.client))
    }
}

fn bump(counter: &mut u32) -> u32 {
    let id = *counter;
    *counter += 1;
    id
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
