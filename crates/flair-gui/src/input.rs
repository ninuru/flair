use flair_core::key::{Key, KeyInput, Mods, NamedKey};
use winit::keyboard::{Key as WinitKey, KeyLocation, ModifiersState, NamedKey as WinitNamed};

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
            WinitNamed::Space => {
                return Some(KeyInput {
                    key: Key::Char(' '),
                    mods: modifiers(mods),
                    text: Some(" ".into()),
                    repeat,
                    keypad: location == KeyLocation::Numpad,
                });
            }
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
