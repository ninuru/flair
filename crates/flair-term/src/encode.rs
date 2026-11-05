use flair_core::key::{KeyInput, MouseInput};

use crate::terminal::Modes;

pub fn encode_key(key: &KeyInput, modes: &Modes) -> Option<Vec<u8>> {
    todo!()
}

pub fn encode_text(text: &str) -> Vec<u8> {
    todo!()
}

pub fn encode_paste(text: &str, modes: &Modes) -> Vec<u8> {
    todo!()
}

pub fn encode_mouse(mouse: &MouseInput, modes: &Modes) -> Option<Vec<u8>> {
    todo!()
}

pub fn encode_focus(focused: bool, modes: &Modes) -> Option<Vec<u8>> {
    todo!()
}
