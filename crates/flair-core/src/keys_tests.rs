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

fn shifted_punctuation(c: char, unshifted: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
        text: Some(c.to_string()),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: Some(unshifted),
        shifted: Some(c),
        base: Some(unshifted),
        locks: Locks::default(),
    }
}

#[test]
fn quote_and_percent_split_by_resulting_geometry() {
    let config = KeysConfig::default();
    assert_eq!(
        config.binding(&shifted_punctuation('"', '\''), true),
        Some(Command::Split(Axis::Vertical))
    );
    assert_eq!(
        config.binding(&shifted_punctuation('%', '5'), true),
        Some(Command::Split(Axis::Horizontal))
    );
    assert_eq!(config.binding(&shifted_punctuation('"', '\''), false), None);
    assert_eq!(config.binding(&shifted_punctuation('%', '5'), false), None);
}

#[test]
fn quote_and_percent_can_be_rebound_or_unbound() {
    let config = KeysConfig {
        prefix: [
            ("\"".to_owned(), "none".to_owned()),
            ("%".to_owned(), "split-vertical".to_owned()),
        ]
        .into(),
        ..KeysConfig::default()
    };
    assert!(config.validate().is_ok());
    assert_eq!(config.binding(&shifted_punctuation('"', '\''), true), None);
    assert_eq!(
        config.binding(&shifted_punctuation('%', '5'), true),
        Some(Command::Split(Axis::Vertical))
    );
}
