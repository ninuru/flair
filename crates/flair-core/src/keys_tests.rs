use super::*;
use crate::{
    command::CopyCommand,
    key::{KeyAction, Locks},
};

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

fn plain(key: Key) -> KeyInput {
    KeyInput {
        key,
        mods: Mods::default(),
        text: None,
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

#[test]
fn copy_mode_defaults_cancel_on_q_and_escape() {
    let config = KeysConfig::default();
    assert_eq!(
        config.copy_binding(&plain(Key::Char('q'))),
        Some(CopyCommand::Cancel)
    );
    assert_eq!(
        config.copy_binding(&plain(Key::Named(NamedKey::Escape))),
        Some(CopyCommand::Cancel)
    );
    assert_eq!(config.copy_binding(&plain(Key::Char('x'))), None);
    assert_eq!(
        config.binding(&plain(Key::Char('[')), true),
        Some(Command::CopyMode)
    );
    assert_eq!(parse_command("copy-mode"), Some(Command::CopyMode));
    assert_eq!(parse_copy_command("cancel"), Some(CopyCommand::Cancel));
    assert_eq!(parse_copy_command("copy-mode"), None);
}

#[test]
fn copy_mode_table_is_validated_against_copy_commands() {
    let table = |key: &str, command: &str| KeysConfig {
        copy_mode: [(key.to_owned(), command.to_owned())].into(),
        ..KeysConfig::default()
    };
    assert!(table("x", "cancel").validate().is_ok());
    assert!(table("x", "none").validate().is_ok());
    assert!(table("x", "new-window").validate().is_err());
    assert!(table("x", "bogus").validate().is_err());
    assert!(table("C-C-x", "cancel").validate().is_err());
    let prefix = KeysConfig {
        prefix: [("x".to_owned(), "cancel".to_owned())].into(),
        ..KeysConfig::default()
    };
    assert!(prefix.validate().is_err());
}

#[test]
fn copy_mode_overrides_rebind_and_unbind() {
    let config = KeysConfig {
        copy_mode: [
            ("q".to_owned(), "none".to_owned()),
            ("C-c".to_owned(), "cancel".to_owned()),
        ]
        .into(),
        ..KeysConfig::default()
    };
    assert_eq!(config.copy_binding(&plain(Key::Char('q'))), None);
    assert_eq!(
        config.copy_binding(&plain(Key::Named(NamedKey::Escape))),
        Some(CopyCommand::Cancel)
    );
    let ctrl_c = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..plain(Key::Char('c'))
    };
    assert_eq!(config.copy_binding(&ctrl_c), Some(CopyCommand::Cancel));
}
