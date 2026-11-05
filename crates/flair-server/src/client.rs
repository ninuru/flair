use flair_core::{geom::GridSize, ids::ClientId, view::ClientView};

pub(crate) struct ClientState {
    pub id: ClientId,
    pub view: ClientView,
    pub size: GridSize,
    pub awaiting_ack: Option<u64>,
    pub next_seq: u64,
}
