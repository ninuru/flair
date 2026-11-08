use super::*;
use crate::key::{KeyAction, Locks};

#[test]
fn punctuation_ignores_shift_without_discarding_other_modifiers() {
    let chord: KeyChord = "&".parse().unwrap();
    let input = KeyInput {
        key: Key::Char('&'),
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
        text: Some("&".into()),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    };
    assert!(chord.matches(&input));
    assert!(!chord.matches(&KeyInput {
        mods: Mods {
            alt: true,
            ..input.mods
        },
        ..input
    }));
    assert_ne!("C-a".parse::<KeyChord>().unwrap(), "C-A".parse().unwrap());
    assert_eq!(
        "C-S-Enter".parse::<KeyChord>().unwrap(),
        "ctrl-shift-return".parse().unwrap()
    );
    assert!("C--".parse::<KeyChord>().is_ok());
}

#[test]
fn invalid_chords_are_rejected() {
    for value in ["", "ctrl-", "C-C-a", "hyper-a", "F36", "two keys"] {
        assert!(value.parse::<KeyChord>().is_err(), "{value}");
    }
}
