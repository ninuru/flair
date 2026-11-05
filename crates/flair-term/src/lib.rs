#![allow(unused_variables)]

pub mod encode;
mod listener;
mod osc;
pub mod pty;
pub mod snapshot;
pub mod terminal;

pub use encode::{encode_focus, encode_key, encode_mouse, encode_paste, encode_text};
pub use pty::{Pty, SpawnSpec};
pub use terminal::{MarkKind, Modes, MouseMode, TermEvent, Terminal};
