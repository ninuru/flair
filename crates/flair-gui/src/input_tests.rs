use super::*;
#[test]
fn modified_enter_and_repeat_survive_translation() {
    let key = translate(
        &WinitKey::Named(WinitNamed::Enter),
        KeyLocation::Standard,
        Some("\r"),
        ModifiersState::CONTROL | ModifiersState::SHIFT,
        true,
    )
    .unwrap();
    assert!(key.mods.ctrl && key.mods.shift && key.repeat);
    assert_eq!(key.key, Key::Named(NamedKey::Enter));
}
#[test]
fn paste_is_local_and_altgr_keeps_text() {
    let paste = translate(
        &WinitKey::Character("V".into()),
        KeyLocation::Standard,
        Some("V"),
        ModifiersState::CONTROL | ModifiersState::SHIFT,
        false,
    )
    .unwrap();
    assert_eq!(local_action(&paste), Some(LocalAction::Paste));
    let altgr = translate(
        &WinitKey::Character("@".into()),
        KeyLocation::Standard,
        Some("@"),
        ModifiersState::empty(),
        false,
    )
    .unwrap();
    assert_eq!(altgr.text.as_deref(), Some("@"));
    assert!(!altgr.mods.alt && !altgr.mods.ctrl);
}
#[test]
fn dead_key_is_ignored() {
    assert!(
        translate(
            &WinitKey::Dead(Some('^')),
            KeyLocation::Standard,
            None,
            ModifiersState::empty(),
            false
        )
        .is_none()
    );
}

#[test]
fn genuine_control_alt_chord_preserves_both_modifiers() {
    let input = translate(
        &WinitKey::Character("x".into()),
        KeyLocation::Standard,
        Some("x"),
        ModifiersState::CONTROL | ModifiersState::ALT,
        false,
    )
    .unwrap();
    assert!(input.mods.ctrl && input.mods.alt);
}

#[test]
fn control_shift_c_copies_without_forwarding_to_the_shell() {
    let key = KeyInput {
        key: Key::Char('C'),
        mods: Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
        text: None,
        repeat: false,
        keypad: false,
    };
    assert_eq!(local_action(&key), Some(LocalAction::Copy));
    let key = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key
    };
    assert_eq!(local_action(&key), None);
}
