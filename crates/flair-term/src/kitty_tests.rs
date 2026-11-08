use super::*;
use crate::terminal::Terminal;
use flair_core::{
    config::Config,
    prefix::{PrefixMachine, PrefixResult},
    time::Millis,
};
use flair_core::{
    geom::GridSize,
    key::{KeyAction, Locks, Mods},
};

fn key(key: Key, mods: Mods) -> KeyInput {
    KeyInput {
        key,
        mods,
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
fn kitty_report_all_encodes_text_and_plain_controls() {
    let modes = Modes {
        kitty_flags: 8,
        ..Modes::default()
    };
    for (input, expected) in [
        (Key::Char('a'), "\x1b[97u"),
        (Key::Char('界'), "\x1b[30028u"),
        (Key::Named(NamedKey::Enter), "\x1b[13u"),
        (Key::Named(NamedKey::Tab), "\x1b[9u"),
        (Key::Named(NamedKey::Backspace), "\x1b[127u"),
    ] {
        assert_eq!(
            encode_key(&key(input, Mods::default()), &modes).unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn kitty_associated_text_has_unicode_codepoints_without_controls() {
    let modes = Modes {
        kitty_flags: 24,
        ..Modes::default()
    };
    let mut input = key(
        Key::Char('a'),
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    input.text = Some("A界\u{301}\r\u{7f}\u{85}".into());
    assert_eq!(
        encode_key(&input, &modes).unwrap(),
        b"\x1b[97;2;65:30028:769u"
    );
    input.text = Some("\r".into());
    assert_eq!(encode_key(&input, &modes).unwrap(), b"\x1b[97;2u");
}

#[test]
fn kitty_report_events_preserves_legacy_text_but_marks_function_repeats() {
    let modes = Modes {
        kitty_flags: 2,
        ..Modes::default()
    };
    let mut input = key(Key::Char('a'), Mods::default());
    input.repeat = true;
    assert_eq!(encode_key(&input, &modes).unwrap(), b"a");
    input.key = Key::Named(NamedKey::Up);
    assert_eq!(encode_key(&input, &modes).unwrap(), b"\x1b[1;1:2A");
    input.key = Key::Named(NamedKey::F(3));
    assert_eq!(encode_key(&input, &modes).unwrap(), b"\x1b[13;1:2~");
}

#[test]
fn kitty_keypad_and_extended_function_keys_have_distinct_codes() {
    let modes = Modes {
        kitty_flags: 1,
        ..Modes::default()
    };
    for (input, expected) in [
        (Key::Char('0'), "\x1b[57399u"),
        (Key::Char('9'), "\x1b[57408u"),
        (Key::Char('.'), "\x1b[57409u"),
        (Key::Char('/'), "\x1b[57410u"),
        (Key::Char('*'), "\x1b[57411u"),
        (Key::Char('-'), "\x1b[57412u"),
        (Key::Char('+'), "\x1b[57413u"),
        (Key::Named(NamedKey::Enter), "\x1b[57414u"),
        (Key::Char('='), "\x1b[57415u"),
        (Key::Char(','), "\x1b[57416u"),
        (Key::Named(NamedKey::Left), "\x1b[57417u"),
        (Key::Named(NamedKey::Delete), "\x1b[57426u"),
    ] {
        let mut input = key(input, Mods::default());
        input.keypad = true;
        assert_eq!(encode_key(&input, &modes).unwrap(), expected.as_bytes());
    }
    for (n, expected) in [(13, "\x1b[57376u"), (35, "\x1b[57398u")] {
        assert_eq!(
            encode_key(&key(Key::Named(NamedKey::F(n)), Mods::default()), &modes).unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn kitty_ime_commits_encode_text_without_a_key_only_when_requested() {
    for flags in [0, 1, 8, 16] {
        let modes = Modes {
            kitty_flags: flags,
            ..Modes::default()
        };
        assert_eq!(
            encode_text_with_modes("日本\u{301}", &modes),
            "日本\u{301}".as_bytes()
        );
    }
    let modes = Modes {
        kitty_flags: 24,
        ..Modes::default()
    };
    assert_eq!(
        encode_text_with_modes("日本\u{301}", &modes),
        b"\x1b[0;;26085:26412:769u"
    );
    assert_eq!(encode_text_with_modes("\r\u{7f}\u{85}", &modes), b"");
    assert_eq!(encode_text_with_modes("", &modes), b"");
}

#[test]
fn kitty_alternate_keys_keep_layout_identity_and_shifted_punctuation() {
    let mut input = key(
        Key::Char('+'),
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
    );
    input.unshifted = Some('=');
    input.shifted = Some('+');
    input.base = Some('=');
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 5,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[61:43:61;6u"
    );
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 1,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[61;6u"
    );
    input.key = Key::Char('с');
    input.unshifted = Some('с');
    input.shifted = Some('С');
    input.base = Some('c');
    input.mods.shift = false;
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 5,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[1089::99;5u"
    );
}

#[test]
fn kitty_releases_require_event_reporting_and_controls_require_all_keys() {
    let mut input = key(Key::Char('a'), Mods::default());
    input.action = KeyAction::Release;
    for flags in [0, 1, 4, 8, 16] {
        assert!(
            encode_key(
                &input,
                &Modes {
                    kitty_flags: flags,
                    ..Modes::default()
                }
            )
            .is_none()
        );
    }
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 10,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[97;1:3u"
    );
    input.key = Key::Named(NamedKey::Enter);
    input.mods.ctrl = true;
    assert!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 3,
                ..Modes::default()
            }
        )
        .is_none()
    );
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 11,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[13;5:3u"
    );
    input.key = Key::Named(NamedKey::Up);
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 2,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[1;5:3A"
    );
}

#[test]
fn kitty_release_omits_associated_text_and_repeat_without_flag_is_press() {
    let mut input = key(Key::Char('a'), Mods::default());
    input.text = Some("a".into());
    input.action = KeyAction::Release;
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 31,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[97;1:3u"
    );
    input.action = KeyAction::Press;
    input.repeat = true;
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 8,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[97u"
    );
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 10,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[97;1:2u"
    );
}

#[test]
fn kitty_lock_modifiers_are_reported_for_escape_codes() {
    let mut input = key(Key::Char('A'), Mods::default());
    input.unshifted = Some('a');
    input.locks = Locks {
        caps: true,
        num: true,
    };
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 1,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"A"
    );
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 8,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[97;193u"
    );
    input.key = Key::Named(NamedKey::Up);
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 1,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[1;193A"
    );
}

#[test]
fn kitty_modifier_keys_require_all_keys_and_functional_keys_have_codes() {
    for (named, expected) in [
        (NamedKey::CapsLock, "\x1b[57358u"),
        (NamedKey::NumLock, "\x1b[57360u"),
        (NamedKey::ShiftLeft, "\x1b[57441u"),
        (NamedKey::ShiftRight, "\x1b[57447u"),
        (NamedKey::ControlLeft, "\x1b[57442u"),
        (NamedKey::ControlRight, "\x1b[57448u"),
        (NamedKey::AltLeft, "\x1b[57443u"),
        (NamedKey::AltRight, "\x1b[57449u"),
        (NamedKey::SuperLeft, "\x1b[57444u"),
        (NamedKey::SuperRight, "\x1b[57450u"),
    ] {
        let input = key(Key::Named(named), Mods::default());
        assert!(encode_key(&input, &Modes::default()).is_none());
        assert!(
            encode_key(
                &input,
                &Modes {
                    kitty_flags: 1,
                    ..Modes::default()
                }
            )
            .is_none()
        );
        assert_eq!(
            encode_key(
                &input,
                &Modes {
                    kitty_flags: 8,
                    ..Modes::default()
                }
            )
            .unwrap(),
            expected.as_bytes()
        );
    }
    for (named, expected) in [
        (NamedKey::ScrollLock, "\x1b[57359u"),
        (NamedKey::PrintScreen, "\x1b[57361u"),
        (NamedKey::Pause, "\x1b[57362u"),
        (NamedKey::Menu, "\x1b[57363u"),
    ] {
        assert_eq!(
            encode_key(
                &key(Key::Named(named), Mods::default()),
                &Modes {
                    kitty_flags: 1,
                    ..Modes::default()
                }
            )
            .unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn kitty_modified_controls_use_canonical_sequences() {
    let modes = Modes {
        kitty_flags: 1,
        ..Modes::default()
    };
    for (named, mods, expected) in [
        (
            NamedKey::Tab,
            Mods {
                shift: true,
                ..Mods::default()
            },
            "\x1b[9;2u",
        ),
        (
            NamedKey::Enter,
            Mods {
                alt: true,
                ..Mods::default()
            },
            "\x1b[13;3u",
        ),
        (
            NamedKey::Backspace,
            Mods {
                alt: true,
                ..Mods::default()
            },
            "\x1b[127;3u",
        ),
    ] {
        assert_eq!(
            encode_key(&key(Key::Named(named), mods), &modes).unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn application_negotiation_controls_encoder_and_restores_each_screen() {
    let mut terminal = Terminal::new(GridSize { cols: 10, rows: 2 }, 0);
    let mut input = key(Key::Char('a'), Mods::default());
    input.text = Some("a".into());
    terminal.advance(b"\x1b[>31u");
    assert_eq!(terminal.modes().kitty_flags, 31);
    assert_eq!(
        encode_key(&input, &terminal.modes()).unwrap(),
        b"\x1b[97;1;97u"
    );
    terminal.advance(b"\x1b[?1049h");
    assert_eq!(terminal.modes().kitty_flags, 0);
    assert_eq!(encode_key(&input, &terminal.modes()).unwrap(), b"a");
    terminal.advance(b"\x1b[>8u");
    assert_eq!(encode_key(&input, &terminal.modes()).unwrap(), b"\x1b[97u");
    terminal.advance(b"\x1b[<u");
    assert_eq!(encode_key(&input, &terminal.modes()).unwrap(), b"a");
    terminal.advance(b"\x1b[?1049l");
    assert_eq!(terminal.modes().kitty_flags, 31);
    terminal.advance(b"\x1b[<u");
    assert_eq!(terminal.modes().kitty_flags, 0);
    assert_eq!(encode_key(&input, &terminal.modes()).unwrap(), b"a");
}

#[test]
fn control_aware_associated_text_does_not_insert_a_printable_shortcut_character() {
    let mut input = key(
        Key::Char('c'),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    input.text = Some("\x03".into());
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 24,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[99;5u"
    );
}

#[test]
fn unbound_application_chord_passes_key_tables_with_full_kitty_modifiers() {
    let config = Config::from_toml("[keys.root]\n'C-S-Enter'='none'").unwrap();
    let mut prefix = PrefixMachine::default();
    let input = key(
        Key::Named(NamedKey::Enter),
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
    );
    assert_eq!(
        prefix.handle(&input, Millis(0), &config.keys),
        PrefixResult::Forward
    );
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 31,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[13;6u"
    );
    let release = KeyInput {
        action: KeyAction::Release,
        ..input
    };
    assert_eq!(
        prefix.handle(&release, Millis(1), &config.keys),
        PrefixResult::Forward
    );
    assert_eq!(
        encode_key(
            &release,
            &Modes {
                kitty_flags: 31,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[13;6:3u"
    );
}

#[test]
fn control_space_does_not_attach_printable_space_as_associated_text() {
    let mut input = key(
        Key::Char(' '),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    input.text = Some("\0".into());
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 24,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[32;5u"
    );
    input.text = Some(" ".into());
    input.mods.ctrl = false;
    assert_eq!(
        encode_key(
            &input,
            &Modes {
                kitty_flags: 24,
                ..Modes::default()
            }
        )
        .unwrap(),
        b"\x1b[32;1;32u"
    );
}
