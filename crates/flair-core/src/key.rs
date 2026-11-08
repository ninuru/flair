use serde::{Deserialize, Serialize};

use crate::geom::CellPoint;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInput {
    pub key: Key,
    pub mods: Mods,
    pub text: Option<String>,
    pub repeat: bool,
    pub keypad: bool,
    pub action: KeyAction,
    pub unshifted: Option<char>,
    pub shifted: Option<char>,
    pub base: Option<char>,
    pub locks: Locks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum KeyAction {
    #[default]
    Press,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Locks {
    pub caps: bool,
    pub num: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NamedKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    Menu,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    SuperLeft,
    SuperRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseInput {
    pub point: CellPoint,
    pub button: MouseButton,
    pub action: MouseAction,
    pub mods: Mods,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
    NoButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    Key(KeyInput),
    Text(String),
    Paste(String),
    Mouse(MouseInput),
    Ime(bool),
}
