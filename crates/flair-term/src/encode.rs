use crate::terminal::{Modes, MouseMode};

#[path = "kitty.rs"]
mod kitty;
use flair_core::key::{Key, KeyAction, KeyInput, MouseAction, MouseButton, MouseInput, NamedKey};

pub fn encode_key(key: &KeyInput, modes: &Modes) -> Option<Vec<u8>> {
    if key.action == KeyAction::Release {
        if modes.kitty_flags & 2 == 0
            || modes.kitty_flags & 8 == 0
                && matches!(
                    key.key,
                    Key::Named(NamedKey::Enter | NamedKey::Tab | NamedKey::Backspace)
                )
        {
            return None;
        }
        return kitty::encode(key, modes);
    }
    let modifier = 1
        + u8::from(key.mods.shift)
        + 2 * u8::from(key.mods.alt)
        + 4 * u8::from(key.mods.ctrl)
        + 8 * u8::from(key.mods.super_);
    if let Some(bytes) = kitty::encode(key, modes) {
        return Some(bytes);
    }
    if key.keypad && modes.app_keypad && modifier == 1 {
        let code = match key.key {
            Key::Char('0') => Some('p'),
            Key::Char('1') => Some('q'),
            Key::Char('2') => Some('r'),
            Key::Char('3') => Some('s'),
            Key::Char('4') => Some('t'),
            Key::Char('5') => Some('u'),
            Key::Char('6') => Some('v'),
            Key::Char('7') => Some('w'),
            Key::Char('8') => Some('x'),
            Key::Char('9') => Some('y'),
            Key::Char('.') => Some('n'),
            Key::Char('+') => Some('k'),
            Key::Char('-') => Some('m'),
            Key::Char('*') => Some('j'),
            Key::Char('/') => Some('o'),
            Key::Named(NamedKey::Enter) => Some('M'),
            _ => None,
        };
        if let Some(code) = code {
            return Some(format!("\x1bO{code}").into_bytes());
        }
    }
    let mut bytes = match key.key {
        Key::Char(c) => {
            if key.mods.ctrl {
                let n = c.to_ascii_uppercase() as u32;
                let code = match n {
                    32 | 64 => 0,
                    65..=95 => (n - 64) as u8,
                    63 => 127,
                    _ => return None,
                };
                vec![code]
            } else {
                key.text
                    .clone()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| c.to_string())
                    .into_bytes()
            }
        }
        Key::Named(named) => match named {
            NamedKey::Enter => vec![b'\r'],
            NamedKey::Tab if key.mods.shift => b"\x1b[Z".to_vec(),
            NamedKey::Tab => vec![b'\t'],
            NamedKey::Backspace => vec![127],
            NamedKey::Escape => vec![27],
            NamedKey::CapsLock
            | NamedKey::NumLock
            | NamedKey::ScrollLock
            | NamedKey::PrintScreen
            | NamedKey::Pause
            | NamedKey::Menu
            | NamedKey::ShiftLeft
            | NamedKey::ShiftRight
            | NamedKey::ControlLeft
            | NamedKey::ControlRight
            | NamedKey::AltLeft
            | NamedKey::AltRight
            | NamedKey::SuperLeft
            | NamedKey::SuperRight => return None,
            NamedKey::Up
            | NamedKey::Down
            | NamedKey::Left
            | NamedKey::Right
            | NamedKey::Home
            | NamedKey::End => {
                let suffix = match named {
                    NamedKey::Up => 'A',
                    NamedKey::Down => 'B',
                    NamedKey::Right => 'C',
                    NamedKey::Left => 'D',
                    NamedKey::Home => 'H',
                    NamedKey::End => 'F',
                    _ => unreachable!(),
                };
                return Some(
                    if modifier != 1 {
                        format!("\x1b[1;{modifier}{suffix}")
                    } else if modes.app_cursor {
                        format!("\x1bO{suffix}")
                    } else {
                        format!("\x1b[{suffix}")
                    }
                    .into_bytes(),
                );
            }
            NamedKey::F(n @ 1..=4) => {
                let suffix = (b'P' + n - 1) as char;
                return Some(
                    if modifier == 1 {
                        format!("\x1bO{suffix}")
                    } else {
                        format!("\x1b[1;{modifier}{suffix}")
                    }
                    .into_bytes(),
                );
            }
            NamedKey::Insert
            | NamedKey::Delete
            | NamedKey::PageUp
            | NamedKey::PageDown
            | NamedKey::F(_) => {
                let code = match named {
                    NamedKey::Insert => 2,
                    NamedKey::Delete => 3,
                    NamedKey::PageUp => 5,
                    NamedKey::PageDown => 6,
                    NamedKey::F(5) => 15,
                    NamedKey::F(6) => 17,
                    NamedKey::F(7) => 18,
                    NamedKey::F(8) => 19,
                    NamedKey::F(9) => 20,
                    NamedKey::F(10) => 21,
                    NamedKey::F(11) => 23,
                    NamedKey::F(12) => 24,
                    _ => return None,
                };
                return Some(
                    if modifier == 1 {
                        format!("\x1b[{code}~")
                    } else {
                        format!("\x1b[{code};{modifier}~")
                    }
                    .into_bytes(),
                );
            }
        },
    };
    if key.mods.alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}

pub fn encode_text(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

pub fn encode_text_with_modes(text: &str, modes: &Modes) -> Vec<u8> {
    if modes.kitty_flags & 24 != 24 {
        return encode_text(text);
    }
    let codepoints = kitty::text_codepoints(text);
    if codepoints.is_empty() {
        Vec::new()
    } else {
        format!("\x1b[0;;{codepoints}u").into_bytes()
    }
}

pub fn encode_paste(text: &str, modes: &Modes) -> Vec<u8> {
    if modes.bracketed_paste {
        format!(
            "\x1b[200~{}\x1b[201~",
            text.replace("\x1b[201~", "").replace('\x1b', "")
        )
        .into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}
pub fn encode_focus(focused: bool, modes: &Modes) -> Option<Vec<u8>> {
    modes.focus_events.then(|| {
        if focused {
            b"\x1b[I".to_vec()
        } else {
            b"\x1b[O".to_vec()
        }
    })
}
pub fn encode_mouse(mouse: &MouseInput, modes: &Modes) -> Option<Vec<u8>> {
    if modes.mouse == MouseMode::Off {
        return None;
    }
    if mouse.action == MouseAction::Motion
        && (modes.mouse == MouseMode::Click
            || (modes.mouse == MouseMode::Drag && mouse.button == MouseButton::NoButton))
    {
        return None;
    }
    let mut code = match mouse.button {
        MouseButton::Left => 0u16,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::NoButton => 3,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
        MouseButton::WheelLeft => 66,
        MouseButton::WheelRight => 67,
    };
    if code >= 64 && mouse.action != MouseAction::Press {
        return None;
    }
    if mouse.action == MouseAction::Motion {
        code += 32;
    }
    code += 4 * u16::from(mouse.mods.shift)
        + 8 * u16::from(mouse.mods.alt)
        + 16 * u16::from(mouse.mods.ctrl);
    let x = u32::from(mouse.point.col) + 1;
    let y = u32::from(mouse.point.row) + 1;
    if modes.sgr_mouse {
        let end = if mouse.action == MouseAction::Release {
            'm'
        } else {
            'M'
        };
        Some(format!("\x1b[<{code};{x};{y}{end}").into_bytes())
    } else {
        if x > 223 || y > 223 {
            return None;
        }
        if mouse.action == MouseAction::Release {
            code = 3;
        }
        Some(vec![
            27,
            b'[',
            b'M',
            code as u8 + 32,
            x as u8 + 32,
            y as u8 + 32,
        ])
    }
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "kitty_tests.rs"]
mod kitty_tests;
