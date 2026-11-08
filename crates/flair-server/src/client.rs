use flair_core::{
    geom::{CellPoint, GridSize},
    ids::PaneId,
    proto::{ClientKind, PaneUpdate, WindowLayout},
    theme::Theme,
    time::Millis,
    view::ClientView,
};

use std::collections::BTreeMap;

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
    pub prefix_until: Option<Millis>,
    pub last_click: Option<(Millis, CellPoint, u8)>,
    pub selecting: bool,
}
