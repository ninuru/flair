use std::{error::Error, fmt, time::Instant};

use flair_core::{config::Config, link::ClientLink};

#[derive(Debug)]
pub struct GuiError(pub String);

impl fmt::Display for GuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for GuiError {}

#[allow(unused_variables)]
pub fn run(config: Config, link: ClientLink, probe: Option<Instant>) -> Result<(), GuiError> {
    todo!()
}
