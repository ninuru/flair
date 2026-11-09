use flair_core::{
    geom::{CellPoint, GridSize},
    ids::{PaneId, SessionId, WindowId},
    key::MouseInput,
    prefix::PrefixMachine,
    proto::{ClientKind, PaneUpdate, WindowLayout},
    status::StatusLine,
    theme::Theme,
    time::Millis,
    view::ClientView,
};

use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct ClientState {
    pub view: ClientView,
    pub kind: ClientKind,
    pub size: GridSize,
    pub awaiting_ack: Option<u64>,
    pub next_seq: u64,
    pub activity: u64,
    pub focused: bool,
    pub full: bool,
    pub previous: BTreeMap<PaneId, PaneUpdate>,
    pub acknowledged: BTreeMap<PaneId, PaneUpdate>,
    pub layout: Option<WindowLayout>,
    pub env: Vec<(String, String)>,
    pub palette: Theme,
    pub prefix: PrefixMachine,
    pub status: Option<StatusLine>,
    pub finished: BTreeSet<SessionId>,
    pub last_click: Option<(Millis, CellPoint, u8)>,
    pub selecting: bool,
    pub mouse: Option<MouseCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MouseCapture {
    Separator { window: WindowId, path: Vec<bool> },
    Selection(PaneId),
    Reporting { pane: PaneId, input: MouseInput },
}
