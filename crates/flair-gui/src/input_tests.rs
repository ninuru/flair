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
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
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

fn complete(
    state: &mut InputState,
    key: WinitKey,
    unshifted: Option<char>,
    physical: KeyCode,
    mods: ModifiersState,
    action: KeyAction,
) -> KeyInput {
    let input = translate(&key, KeyLocation::Standard, None, mods, false).unwrap();
    state.complete(
        input,
        action,
        unshifted,
        base_key(PhysicalKey::Code(physical)),
    )
}

#[test]
fn shifted_punctuation_keeps_logical_binding_and_unshifted_kitty_identity() {
    let mut state = InputState::default();
    let input = complete(
        &mut state,
        WinitKey::Character("&".into()),
        Some('7'),
        KeyCode::Digit7,
        ModifiersState::SHIFT,
        KeyAction::Press,
    );
    assert_eq!(input.key, Key::Char('&'));
    assert_eq!(input.unshifted, Some('7'));
    assert_eq!(input.shifted, Some('&'));
    assert_eq!(input.base, Some('7'));
    assert!(
        "&".parse::<flair_core::keys::KeyChord>()
            .unwrap()
            .matches(&input)
    );
}

#[test]
fn release_preserves_key_identity_and_never_fires_local_action() {
    let mut state = InputState::default();
    let input = complete(
        &mut state,
        WinitKey::Character("V".into()),
        Some('v'),
        KeyCode::KeyV,
        ModifiersState::CONTROL | ModifiersState::SHIFT,
        KeyAction::Release,
    );
    assert_eq!(input.action, KeyAction::Release);
    assert_eq!(input.unshifted, Some('v'));
    assert_eq!(input.text, None);
    assert_eq!(local_action(&input), None);
}

#[test]
fn cyrillic_logical_key_has_us_physical_alternative() {
    let mut state = InputState::default();
    let input = complete(
        &mut state,
        WinitKey::Character("с".into()),
        Some('с'),
        KeyCode::KeyC,
        ModifiersState::CONTROL,
        KeyAction::Press,
    );
    assert_eq!(input.key, Key::Char('с'));
    assert_eq!(input.base, Some('c'));
}

#[test]
fn lock_bits_follow_toggle_events_and_initial_caps_state() {
    let mut state = InputState::default();
    let caps = complete(
        &mut state,
        WinitKey::Named(WinitNamed::CapsLock),
        None,
        KeyCode::CapsLock,
        ModifiersState::empty(),
        KeyAction::Press,
    );
    assert!(caps.locks.caps);
    let num = complete(
        &mut state,
        WinitKey::Named(WinitNamed::NumLock),
        None,
        KeyCode::NumLock,
        ModifiersState::empty(),
        KeyAction::Press,
    );
    assert!(num.locks.num);
    let letter = complete(
        &mut state,
        WinitKey::Character("A".into()),
        Some('a'),
        KeyCode::KeyA,
        ModifiersState::empty(),
        KeyAction::Press,
    );
    assert!(letter.locks.caps && letter.locks.num);
}

#[test]
fn modifier_events_apply_current_state_and_keep_other_side_held() {
    let mut state = InputState::default();
    let left = translate(
        &WinitKey::Named(WinitNamed::Shift),
        KeyLocation::Left,
        None,
        ModifiersState::empty(),
        false,
    )
    .unwrap();
    let right = translate(
        &WinitKey::Named(WinitNamed::Shift),
        KeyLocation::Right,
        None,
        ModifiersState::SHIFT,
        false,
    )
    .unwrap();
    assert!(
        state
            .complete(left.clone(), KeyAction::Press, None, None)
            .mods
            .shift
    );
    assert!(
        state
            .complete(right.clone(), KeyAction::Press, None, None)
            .mods
            .shift
    );
    assert!(
        state
            .complete(left, KeyAction::Release, None, None)
            .mods
            .shift
    );
    assert!(
        !state
            .complete(right, KeyAction::Release, None, None)
            .mods
            .shift
    );
}

#[test]
fn extended_function_keys_survive_translation() {
    for (key, number) in [(WinitNamed::F13, 13), (WinitNamed::F35, 35)] {
        assert_eq!(
            translate(
                &WinitKey::Named(key),
                KeyLocation::Standard,
                None,
                ModifiersState::empty(),
                false
            )
            .unwrap()
            .key,
            Key::Named(NamedKey::F(number))
        );
    }
}

#[test]
fn control_aware_text_survives_translation_without_fabricating_printable_text() {
    let input = translate(
        &WinitKey::Character("c".into()),
        KeyLocation::Standard,
        Some("\x03"),
        ModifiersState::CONTROL,
        false,
    )
    .unwrap();
    assert_eq!(input.key, Key::Char('c'));
    assert_eq!(input.text.as_deref(), Some("\x03"));
}

#[test]
fn named_space_preserves_control_aware_text_and_missing_release_text() {
    for text in [Some("\0"), Some(" "), None] {
        let input = translate(
            &WinitKey::Named(WinitNamed::Space),
            KeyLocation::Standard,
            text,
            ModifiersState::CONTROL,
            false,
        )
        .unwrap();
        assert_eq!(input.key, Key::Char(' '));
        assert_eq!(input.text.as_deref(), text);
    }
}
