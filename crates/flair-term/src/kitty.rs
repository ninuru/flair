use flair_core::key::{Key, KeyAction, KeyInput, NamedKey};

use crate::terminal::Modes;

pub(super) fn encode(key: &KeyInput, modes: &Modes) -> Option<Vec<u8>> {
    let flags = modes.kitty_flags;
    let all = flags & 8 != 0;
    let disambiguate = all || flags & 1 != 0;
    let events = flags & 2 != 0;
    let released = key.action == KeyAction::Release;
    if flags & 11 == 0 {
        return None;
    }
    let modified = key.mods.ctrl || key.mods.alt || key.mods.super_;
    let encode = match key.key {
        Key::Char(_) => all || disambiguate && (modified || key.keypad),
        Key::Named(NamedKey::Escape) => disambiguate || events && released,
        Key::Named(NamedKey::Enter) => {
            all || disambiguate && (key.keypad || key.mods.shift || modified)
        }
        Key::Named(NamedKey::Tab | NamedKey::Backspace) => {
            all || disambiguate && (key.mods.shift || modified)
        }
        Key::Named(
            NamedKey::CapsLock
            | NamedKey::NumLock
            | NamedKey::ShiftLeft
            | NamedKey::ShiftRight
            | NamedKey::ControlLeft
            | NamedKey::ControlRight
            | NamedKey::AltLeft
            | NamedKey::AltRight
            | NamedKey::SuperLeft
            | NamedKey::SuperRight,
        ) => all,
        Key::Named(_) => true,
    };
    if !encode {
        return None;
    }
    let (code, suffix) = key_code(key, disambiguate)?;
    let modifier = 1
        + u16::from(key.mods.shift)
        + 2 * u16::from(key.mods.alt)
        + 4 * u16::from(key.mods.ctrl)
        + 8 * u16::from(key.mods.super_)
        + 64 * u16::from(key.locks.caps)
        + 128 * u16::from(key.locks.num);
    let text = if all && flags & 16 != 0 && !released {
        text_codepoints(key.text.as_deref().unwrap_or_default())
    } else {
        String::new()
    };
    let mut sequence = format!("\x1b[{code}");
    if flags & 4 != 0 && suffix == 'u' && !key.keypad && matches!(key.key, Key::Char(_)) {
        let shifted = key.shifted.filter(|_| key.mods.shift);
        if shifted.is_some() || key.base.is_some() {
            sequence.push(':');
            if let Some(shifted) = shifted {
                sequence.push_str(&(shifted as u32).to_string());
            }
            if let Some(base) = key.base {
                sequence.push(':');
                sequence.push_str(&(base as u32).to_string());
            }
        }
    }
    let event = if released {
        3
    } else if key.repeat {
        2
    } else {
        1
    };
    let report_event = events && event != 1;
    if modifier != 1 || report_event || !text.is_empty() {
        sequence.push(';');
        sequence.push_str(&modifier.to_string());
        if report_event {
            sequence.push(':');
            sequence.push_str(&event.to_string());
        }
    }
    if !text.is_empty() {
        sequence.push(';');
        sequence.push_str(&text);
    }
    if code == 1 && suffix != 'u' && modifier == 1 && !report_event && text.is_empty() {
        sequence.truncate(2);
    }
    sequence.push(suffix);
    Some(sequence.into_bytes())
}

pub(super) fn text_codepoints(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .map(|c| (c as u32).to_string())
        .collect::<Vec<_>>()
        .join(":")
}

fn key_code(key: &KeyInput, disambiguate: bool) -> Option<(u32, char)> {
    if key.keypad && disambiguate {
        let code = match key.key {
            Key::Char(c @ '0'..='9') => Some(57399 + c as u32 - '0' as u32),
            Key::Char('.') => Some(57409),
            Key::Char('/') => Some(57410),
            Key::Char('*') => Some(57411),
            Key::Char('-') => Some(57412),
            Key::Char('+') => Some(57413),
            Key::Named(NamedKey::Enter) => Some(57414),
            Key::Char('=') => Some(57415),
            Key::Char(',') => Some(57416),
            Key::Named(NamedKey::Left) => Some(57417),
            Key::Named(NamedKey::Right) => Some(57418),
            Key::Named(NamedKey::Up) => Some(57419),
            Key::Named(NamedKey::Down) => Some(57420),
            Key::Named(NamedKey::PageUp) => Some(57421),
            Key::Named(NamedKey::PageDown) => Some(57422),
            Key::Named(NamedKey::Home) => Some(57423),
            Key::Named(NamedKey::End) => Some(57424),
            Key::Named(NamedKey::Insert) => Some(57425),
            Key::Named(NamedKey::Delete) => Some(57426),
            Key::Char(_) | Key::Named(_) => None,
        };
        if let Some(code) = code {
            return Some((code, 'u'));
        }
    }
    Some(match key.key {
        Key::Char(c) => (key.unshifted.unwrap_or(c.to_ascii_lowercase()) as u32, 'u'),
        Key::Named(named) => match named {
            NamedKey::Escape => (27, 'u'),
            NamedKey::Enter => (13, 'u'),
            NamedKey::Tab => (9, 'u'),
            NamedKey::Backspace => (127, 'u'),
            NamedKey::Insert => (2, '~'),
            NamedKey::Delete => (3, '~'),
            NamedKey::Left => (1, 'D'),
            NamedKey::Right => (1, 'C'),
            NamedKey::Up => (1, 'A'),
            NamedKey::Down => (1, 'B'),
            NamedKey::PageUp => (5, '~'),
            NamedKey::PageDown => (6, '~'),
            NamedKey::Home => (1, 'H'),
            NamedKey::End => (1, 'F'),
            NamedKey::F(1) => (1, 'P'),
            NamedKey::F(2) => (1, 'Q'),
            NamedKey::F(3) => (13, '~'),
            NamedKey::F(4) => (1, 'S'),
            NamedKey::F(5) => (15, '~'),
            NamedKey::F(n @ 6..=10) => (u32::from(n) + 11, '~'),
            NamedKey::F(n @ 11..=12) => (u32::from(n) + 12, '~'),
            NamedKey::F(n @ 13..=35) => (u32::from(n) + 57363, 'u'),
            NamedKey::F(_) => return None,
            NamedKey::CapsLock => (57358, 'u'),
            NamedKey::ScrollLock => (57359, 'u'),
            NamedKey::NumLock => (57360, 'u'),
            NamedKey::PrintScreen => (57361, 'u'),
            NamedKey::Pause => (57362, 'u'),
            NamedKey::Menu => (57363, 'u'),
            NamedKey::ShiftLeft => (57441, 'u'),
            NamedKey::ControlLeft => (57442, 'u'),
            NamedKey::AltLeft => (57443, 'u'),
            NamedKey::SuperLeft => (57444, 'u'),
            NamedKey::ShiftRight => (57447, 'u'),
            NamedKey::ControlRight => (57448, 'u'),
            NamedKey::AltRight => (57449, 'u'),
            NamedKey::SuperRight => (57450, 'u'),
        },
    })
}
