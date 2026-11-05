use std::sync::mpsc::{Receiver, Sender, channel};

use crate::proto::{ClientMsg, ServerMsg};

pub struct ClientLink {
    pub tx: Sender<ClientMsg>,
    pub rx: Receiver<ServerMsg>,
}

pub struct ServerLink {
    pub tx: Sender<ServerMsg>,
    pub rx: Receiver<ClientMsg>,
}

pub fn channel_pair() -> (ClientLink, ServerLink) {
    let (client_tx, server_rx) = channel();
    let (server_tx, client_rx) = channel();
    (
        ClientLink {
            tx: client_tx,
            rx: client_rx,
        },
        ServerLink {
            tx: server_tx,
            rx: server_rx,
        },
    )
}
