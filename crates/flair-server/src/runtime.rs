use std::{io, thread::JoinHandle};

use flair_core::link::ServerLink;

use crate::config::ServerConfig;

pub fn spawn_in_process(config: ServerConfig, link: ServerLink) -> io::Result<JoinHandle<()>> {
    todo!()
}
