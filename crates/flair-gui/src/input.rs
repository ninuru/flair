use std::collections::HashSet;

use flair_core::key::{Key, KeyAction, KeyInput, Locks, Mods, NamedKey};
use winit::keyboard::{
    Key as WinitKey, KeyCode, KeyLocation, ModifiersState, NamedKey as WinitNamed, PhysicalKey,
};
use winit::{
    event::{ElementState, KeyEvent},
    platform::modifier_supplement::KeyEventExtModifierSupplement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalAction {
    Paste,
    Copy,
    ScrollPageUp,
    ScrollPageDown,
}

pub fn translate(
    key: &WinitKey,
    location: KeyLocation,
    text: Option<&str>,
    mods: ModifiersState,
    repeat: bool,
) -> Option<KeyInput> {
    let key = match key {
        WinitKey::Character(value) => Key::Char(value.chars().next()?),
        WinitKey::Named(WinitNamed::Space) => Key::Char(' '),
        WinitKey::Named(named) => Key::Named(match named {
            WinitNamed::Enter => NamedKey::Enter,
            WinitNamed::Tab => NamedKey::Tab,
            WinitNamed::Backspace => NamedKey::Backspace,
            WinitNamed::Escape => NamedKey::Escape,
            WinitNamed::ArrowUp => NamedKey::Up,
            WinitNamed::ArrowDown => NamedKey::Down,
            WinitNamed::ArrowLeft => NamedKey::Left,
            WinitNamed::ArrowRight => NamedKey::Right,
            WinitNamed::Home => NamedKey::Home,
            WinitNamed::End => NamedKey::End,
            WinitNamed::PageUp => NamedKey::PageUp,
            WinitNamed::PageDown => NamedKey::PageDown,
            WinitNamed::Insert => NamedKey::Insert,
            WinitNamed::Delete => NamedKey::Delete,
            WinitNamed::F1 => NamedKey::F(1),
            WinitNamed::F2 => NamedKey::F(2),
            WinitNamed::F3 => NamedKey::F(3),
            WinitNamed::F4 => NamedKey::F(4),
            WinitNamed::F5 => NamedKey::F(5),
            WinitNamed::F6 => NamedKey::F(6),
            WinitNamed::F7 => NamedKey::F(7),
            WinitNamed::F8 => NamedKey::F(8),
            WinitNamed::F9 => NamedKey::F(9),
            WinitNamed::F10 => NamedKey::F(10),
            WinitNamed::F11 => NamedKey::F(11),
            WinitNamed::F12 => NamedKey::F(12),
            WinitNamed::F13 => NamedKey::F(13),
            WinitNamed::F14 => NamedKey::F(14),
            WinitNamed::F15 => NamedKey::F(15),
            WinitNamed::F16 => NamedKey::F(16),
            WinitNamed::F17 => NamedKey::F(17),
            WinitNamed::F18 => NamedKey::F(18),
            WinitNamed::F19 => NamedKey::F(19),
            WinitNamed::F20 => NamedKey::F(20),
            WinitNamed::F21 => NamedKey::F(21),
            WinitNamed::F22 => NamedKey::F(22),
            WinitNamed::F23 => NamedKey::F(23),
            WinitNamed::F24 => NamedKey::F(24),
            WinitNamed::F25 => NamedKey::F(25),
            WinitNamed::F26 => NamedKey::F(26),
            WinitNamed::F27 => NamedKey::F(27),
            WinitNamed::F28 => NamedKey::F(28),
            WinitNamed::F29 => NamedKey::F(29),
            WinitNamed::F30 => NamedKey::F(30),
            WinitNamed::F31 => NamedKey::F(31),
            WinitNamed::F32 => NamedKey::F(32),
            WinitNamed::F33 => NamedKey::F(33),
            WinitNamed::F34 => NamedKey::F(34),
            WinitNamed::F35 => NamedKey::F(35),
            WinitNamed::CapsLock => NamedKey::CapsLock,
            WinitNamed::NumLock => NamedKey::NumLock,
            WinitNamed::ScrollLock => NamedKey::ScrollLock,
            WinitNamed::PrintScreen => NamedKey::PrintScreen,
            WinitNamed::Pause => NamedKey::Pause,
            WinitNamed::ContextMenu => NamedKey::Menu,
            WinitNamed::Shift => {
                if location == KeyLocation::Right {
                    NamedKey::ShiftRight
                } else {
                    NamedKey::ShiftLeft
                }
            }
            WinitNamed::Control => {
                if location == KeyLocation::Right {
                    NamedKey::ControlRight
                } else {
                    NamedKey::ControlLeft
                }
            }
            WinitNamed::Alt => {
                if location == KeyLocation::Right {
                    NamedKey::AltRight
                } else {
                    NamedKey::AltLeft
                }
            }
            WinitNamed::Super => {
                if location == KeyLocation::Right {
                    NamedKey::SuperRight
                } else {
                    NamedKey::SuperLeft
                }
            }
            _ => return None,
        }),
        WinitKey::Dead(_) | WinitKey::Unidentified(_) => return None,
    };
    let mods = modifiers(mods);
    let text = text.filter(|value| !value.is_empty()).map(str::to_owned);
    Some(KeyInput {
        key,
        mods,
        text,
        repeat,
        keypad: location == KeyLocation::Numpad,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    })
}

#[derive(Debug, Default)]
pub struct InputState {
    locks: Locks,
    held_modifiers: HashSet<NamedKey>,
}

impl InputState {
    pub fn translate(&mut self, event: &KeyEvent, mods: ModifiersState) -> Option<KeyInput> {
        let input = translate(
            &event.logical_key,
            event.location,
            event.text_with_all_modifiers(),
            mods,
            event.repeat,
        )?;
        let unshifted = match event.key_without_modifiers() {
            WinitKey::Character(text) => text.chars().next(),
            WinitKey::Named(_) | WinitKey::Dead(_) | WinitKey::Unidentified(_) => None,
        };
        let action = match event.state {
            ElementState::Pressed => KeyAction::Press,
            ElementState::Released => KeyAction::Release,
        };
        Some(self.complete(input, action, unshifted, base_key(event.physical_key)))
    }

    fn complete(
        &mut self,
        mut input: KeyInput,
        action: KeyAction,
        unshifted: Option<char>,
        base: Option<char>,
    ) -> KeyInput {
        input.action = action;
        input.unshifted = unshifted;
        input.base = base;
        if let Key::Char(c) = input.key {
            if input.mods.shift && Some(c) != unshifted {
                input.shifted = Some(c);
            }
            if c.is_ascii_alphabetic() && action == KeyAction::Press {
                self.locks.caps = c.is_ascii_uppercase() != input.mods.shift;
            }
            if input.keypad && c.is_ascii_digit() {
                self.locks.num = !input.mods.shift;
            }
        }
        if let Key::Named(named) = input.key {
            if action == KeyAction::Press && !input.repeat {
                if named == NamedKey::CapsLock {
                    self.locks.caps = !self.locks.caps;
                } else if named == NamedKey::NumLock {
                    self.locks.num = !self.locks.num;
                }
            }
            self.modifier_event(named, &mut input);
        }
        if action == KeyAction::Release {
            input.text = None;
        }
        input.locks = self.locks;
        input
    }

    fn modifier_event(&mut self, named: NamedKey, input: &mut KeyInput) {
        let (left, right, value) = if matches!(named, NamedKey::ShiftLeft | NamedKey::ShiftRight) {
            (
                NamedKey::ShiftLeft,
                NamedKey::ShiftRight,
                &mut input.mods.shift,
            )
        } else if matches!(named, NamedKey::ControlLeft | NamedKey::ControlRight) {
            (
                NamedKey::ControlLeft,
                NamedKey::ControlRight,
                &mut input.mods.ctrl,
            )
        } else if matches!(named, NamedKey::AltLeft | NamedKey::AltRight) {
            (NamedKey::AltLeft, NamedKey::AltRight, &mut input.mods.alt)
        } else if matches!(named, NamedKey::SuperLeft | NamedKey::SuperRight) {
            (
                NamedKey::SuperLeft,
                NamedKey::SuperRight,
                &mut input.mods.super_,
            )
        } else {
            return;
        };
        match input.action {
            KeyAction::Press => {
                self.held_modifiers.insert(named);
            }
            KeyAction::Release => {
                self.held_modifiers.remove(&named);
            }
        }
        *value = self.held_modifiers.contains(&left) || self.held_modifiers.contains(&right);
    }
}

fn base_key(physical: PhysicalKey) -> Option<char> {
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    Some(match code {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Backquote => '`',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        KeyCode::Space => ' ',
        _ => return None,
    })
}

fn modifiers(mods: ModifiersState) -> Mods {
    Mods {
        shift: mods.shift_key(),
        ctrl: mods.control_key(),
        alt: mods.alt_key(),
        super_: mods.super_key(),
    }
}

pub fn local_action(key: &KeyInput) -> Option<LocalAction> {
    if key.action != KeyAction::Press {
        return None;
    }
    if key.mods.ctrl
        && key.mods.shift
        && !key.mods.alt
        && !key.mods.super_
        && matches!(key.key, Key::Char('c' | 'C'))
    {
        return Some(LocalAction::Copy);
    }
    if key.mods.ctrl
        && key.mods.shift
        && !key.mods.alt
        && !key.mods.super_
        && matches!(key.key, Key::Char('v' | 'V'))
    {
        return Some(LocalAction::Paste);
    }
    if key.mods.shift && !key.mods.ctrl && !key.mods.alt && !key.mods.super_ {
        match key.key {
            Key::Named(NamedKey::PageUp) => return Some(LocalAction::ScrollPageUp),
            Key::Named(NamedKey::PageDown) => return Some(LocalAction::ScrollPageDown),
            Key::Char(_) | Key::Named(_) => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
