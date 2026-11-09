use std::time::Duration;

use flair_core::key::KeyInput;

pub enum Drive {
    Key(KeyInput),
    Text(String),
    Resize { width: u32, height: u32 },
    Sleep(Duration),
}
