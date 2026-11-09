use super::*;
use crate::{
    command::{CopyCommand, CopyMotion, SelectionKind},
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

fn with_mods(key: Key, ctrl: bool, shift: bool) -> KeyInput {
    KeyInput {
        mods: Mods {
            ctrl,
            shift,
            ..Mods::default()
        },
        ..plain(key)
    }
}

#[test]
fn copy_mode_defaults_cover_motions_selection_and_search() {
    let config = KeysConfig::default();
    let motion = CopyCommand::Move;
    for (key, ctrl, shift, expected) in [
        (Key::Char('h'), false, false, motion(CopyMotion::Left)),
        (Key::Char('j'), false, false, motion(CopyMotion::Down)),
        (Key::Char('k'), false, false, motion(CopyMotion::Up)),
        (Key::Char('l'), false, false, motion(CopyMotion::Right)),
        (
            Key::Named(NamedKey::Left),
            false,
            false,
            motion(CopyMotion::Left),
        ),
        (
            Key::Named(NamedKey::Down),
            false,
            false,
            motion(CopyMotion::Down),
        ),
        (
            Key::Named(NamedKey::Up),
            false,
            false,
            motion(CopyMotion::Up),
        ),
        (
            Key::Named(NamedKey::Right),
            false,
            false,
            motion(CopyMotion::Right),
        ),
        (Key::Char('w'), false, false, motion(CopyMotion::NextWord)),
        (
            Key::Char('b'),
            false,
            false,
            motion(CopyMotion::PreviousWord),
        ),
        (
            Key::Char('e'),
            false,
            false,
            motion(CopyMotion::NextWordEnd),
        ),
        (Key::Char('W'), false, true, motion(CopyMotion::NextSpace)),
        (
            Key::Char('B'),
            false,
            true,
            motion(CopyMotion::PreviousSpace),
        ),
        (
            Key::Char('E'),
            false,
            true,
            motion(CopyMotion::NextSpaceEnd),
        ),
        (
            Key::Char('0'),
            false,
            false,
            motion(CopyMotion::StartOfLine),
        ),
        (
            Key::Char('^'),
            false,
            true,
            motion(CopyMotion::BackToIndentation),
        ),
        (Key::Char('$'), false, true, motion(CopyMotion::EndOfLine)),
        (Key::Char('g'), false, false, motion(CopyMotion::HistoryTop)),
        (
            Key::Char('G'),
            false,
            true,
            motion(CopyMotion::HistoryBottom),
        ),
        (Key::Char('u'), true, false, motion(CopyMotion::HalfPageUp)),
        (
            Key::Char('d'),
            true,
            false,
            motion(CopyMotion::HalfPageDown),
        ),
        (Key::Char('H'), false, true, motion(CopyMotion::TopLine)),
        (Key::Char('M'), false, true, motion(CopyMotion::MiddleLine)),
        (Key::Char('L'), false, true, motion(CopyMotion::BottomLine)),
        (
            Key::Char('v'),
            false,
            false,
            CopyCommand::Select(SelectionKind::Simple),
        ),
        (
            Key::Char('V'),
            false,
            true,
            CopyCommand::Select(SelectionKind::Lines),
        ),
        (
            Key::Char('v'),
            true,
            false,
            CopyCommand::Select(SelectionKind::Block),
        ),
        (Key::Char('o'), false, false, CopyCommand::OtherEnd),
        (
            Key::Char('/'),
            false,
            false,
            CopyCommand::SearchPrompt { forward: true },
        ),
        (
            Key::Char('?'),
            false,
            true,
            CopyCommand::SearchPrompt { forward: false },
        ),
        (Key::Char('n'), false, false, CopyCommand::SearchAgain),
        (Key::Char('N'), false, true, CopyCommand::SearchReverse),
        (Key::Char('y'), false, false, CopyCommand::Yank),
        (Key::Named(NamedKey::Enter), false, false, CopyCommand::Yank),
    ] {
        assert_eq!(
            config.copy_binding(&with_mods(key, ctrl, shift)),
            Some(expected),
            "{key:?}"
        );
    }
    assert_eq!(
        config.copy_binding(&with_mods(Key::Char('w'), false, true)),
        Some(motion(CopyMotion::NextSpace))
    );
    assert_eq!(
        config.copy_binding(&with_mods(Key::Char('j'), true, false)),
        None
    );
}

#[test]
fn copy_mode_command_names_follow_tmux() {
    for (name, expected) in [
        (
            "next-space-end",
            CopyCommand::Move(CopyMotion::NextSpaceEnd),
        ),
        (
            "back-to-indentation",
            CopyCommand::Move(CopyMotion::BackToIndentation),
        ),
        ("halfpage-down", CopyCommand::Move(CopyMotion::HalfPageDown)),
        (
            "rectangle-toggle",
            CopyCommand::Select(SelectionKind::Block),
        ),
        ("other-end", CopyCommand::OtherEnd),
        (
            "search-backward",
            CopyCommand::SearchPrompt { forward: false },
        ),
        ("search-reverse", CopyCommand::SearchReverse),
        ("copy-selection-and-cancel", CopyCommand::Yank),
    ] {
        assert_eq!(parse_copy_command(name), Some(expected));
    }
    let config = KeysConfig {
        copy_mode: [("x".to_owned(), "select-line".to_owned())].into(),
        ..KeysConfig::default()
    };
    assert!(config.validate().is_ok());
    assert_eq!(
        config.copy_binding(&plain(Key::Char('x'))),
        Some(CopyCommand::Select(SelectionKind::Lines))
    );
}
