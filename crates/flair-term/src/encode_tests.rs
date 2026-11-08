use super::*;
use flair_core::geom::CellPoint;
use flair_core::key::{KeyAction, Locks, Mods, MouseAction, MouseButton};

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
fn legacy_characters_and_controls() {
    let modes = Modes::default();
    let cases = [
        (Key::Char('a'), Mods::default(), "a"),
        (Key::Char('界'), Mods::default(), "界"),
        (
            Key::Char('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            "\x01",
        ),
        (
            Key::Char(' '),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            "\0",
        ),
        (
            Key::Char('['),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            "\x1b",
        ),
        (
            Key::Char('?'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            "\x7f",
        ),
        (
            Key::Char('x'),
            Mods {
                alt: true,
                ..Mods::default()
            },
            "\x1bx",
        ),
        (
            Key::Char('X'),
            Mods {
                alt: true,
                shift: true,
                ..Mods::default()
            },
            "\x1bX",
        ),
        (Key::Named(NamedKey::Enter), Mods::default(), "\r"),
        (Key::Named(NamedKey::Tab), Mods::default(), "\t"),
        (
            Key::Named(NamedKey::Tab),
            Mods {
                shift: true,
                ..Mods::default()
            },
            "\x1b[Z",
        ),
        (Key::Named(NamedKey::Backspace), Mods::default(), "\x7f"),
        (Key::Named(NamedKey::Escape), Mods::default(), "\x1b"),
        (
            Key::Named(NamedKey::Enter),
            Mods {
                ctrl: true,
                shift: true,
                ..Mods::default()
            },
            "\r",
        ),
        (
            Key::Named(NamedKey::Enter),
            Mods {
                alt: true,
                shift: true,
                ..Mods::default()
            },
            "\x1b\r",
        ),
    ];
    for (input, mods, expected) in cases {
        assert_eq!(
            encode_key(&key(input, mods), &modes).unwrap(),
            expected.as_bytes(),
            "{input:?} {mods:?}"
        );
    }
    let mut input = key(Key::Char('e'), Mods::default());
    input.text = Some("é".into());
    input.repeat = true;
    assert_eq!(encode_key(&input, &modes).unwrap(), "é".as_bytes());
    assert_eq!(encode_text("界\r\n"), "界\r\n".as_bytes());
}

#[test]
fn arrow_and_navigation_modifiers() {
    for (named, suffix) in [
        (NamedKey::Up, 'A'),
        (NamedKey::Down, 'B'),
        (NamedKey::Right, 'C'),
        (NamedKey::Left, 'D'),
        (NamedKey::Home, 'H'),
        (NamedKey::End, 'F'),
    ] {
        for app_cursor in [false, true] {
            let modes = Modes {
                app_cursor,
                ..Modes::default()
            };
            let plain = if app_cursor {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1b[{suffix}")
            };
            assert_eq!(
                encode_key(&key(Key::Named(named), Mods::default()), &modes).unwrap(),
                plain.into_bytes()
            );
            assert_eq!(
                encode_key(
                    &key(
                        Key::Named(named),
                        Mods {
                            ctrl: true,
                            ..Mods::default()
                        }
                    ),
                    &modes
                )
                .unwrap(),
                format!("\x1b[1;5{suffix}").into_bytes()
            );
        }
    }
    for (named, code) in [
        (NamedKey::Insert, 2),
        (NamedKey::Delete, 3),
        (NamedKey::PageUp, 5),
        (NamedKey::PageDown, 6),
    ] {
        for (mods, tail) in [
            (Mods::default(), ""),
            (
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                ";5",
            ),
        ] {
            assert_eq!(
                encode_key(&key(Key::Named(named), mods), &Modes::default()).unwrap(),
                format!("\x1b[{code}{tail}~").into_bytes()
            );
        }
    }
}

#[test]
fn f1_through_f12_with_modifiers() {
    for (mods, modifier) in [
        (Mods::default(), 1),
        (
            Mods {
                shift: true,
                ..Mods::default()
            },
            2,
        ),
        (
            Mods {
                alt: true,
                ..Mods::default()
            },
            3,
        ),
        (
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            5,
        ),
    ] {
        for n in 1..=12 {
            let expected = if n <= 4 {
                let suffix = (b'P' + n - 1) as char;
                if modifier == 1 {
                    format!("\x1bO{suffix}")
                } else {
                    format!("\x1b[1;{modifier}{suffix}")
                }
            } else {
                let code = [15, 17, 18, 19, 20, 21, 23, 24][n as usize - 5];
                if modifier == 1 {
                    format!("\x1b[{code}~")
                } else {
                    format!("\x1b[{code};{modifier}~")
                }
            };
            assert_eq!(
                encode_key(&key(Key::Named(NamedKey::F(n)), mods), &Modes::default()).unwrap(),
                expected.into_bytes()
            );
        }
    }
}

#[test]
fn application_keypad() {
    let modes = Modes {
        app_keypad: true,
        ..Modes::default()
    };
    for (c, suffix) in [
        ('0', 'p'),
        ('1', 'q'),
        ('2', 'r'),
        ('3', 's'),
        ('4', 't'),
        ('5', 'u'),
        ('6', 'v'),
        ('7', 'w'),
        ('8', 'x'),
        ('9', 'y'),
        ('.', 'n'),
        ('+', 'k'),
        ('-', 'm'),
        ('*', 'j'),
        ('/', 'o'),
    ] {
        let mut input = key(Key::Char(c), Mods::default());
        input.keypad = true;
        assert_eq!(
            encode_key(&input, &modes).unwrap(),
            format!("\x1bO{suffix}").into_bytes()
        );
        assert_eq!(
            encode_key(&input, &Modes::default()).unwrap(),
            c.to_string().into_bytes()
        );
    }
    let mut enter = key(Key::Named(NamedKey::Enter), Mods::default());
    enter.keypad = true;
    assert_eq!(encode_key(&enter, &modes).unwrap(), b"\x1bOM");
}

#[test]
fn kitty_disambiguates_modified_enter_and_escape() {
    let modes = Modes {
        kitty_flags: 1,
        ..Modes::default()
    };
    for (named, mods, expected) in [
        (
            NamedKey::Enter,
            Mods {
                ctrl: true,
                shift: true,
                ..Mods::default()
            },
            "\x1b[13;6u",
        ),
        (
            NamedKey::Enter,
            Mods {
                alt: true,
                shift: true,
                ..Mods::default()
            },
            "\x1b[13;4u",
        ),
        (NamedKey::Escape, Mods::default(), "\x1b[27u"),
        (NamedKey::Enter, Mods::default(), "\r"),
    ] {
        assert_eq!(
            encode_key(&key(Key::Named(named), mods), &modes).unwrap(),
            expected.as_bytes()
        );
    }
}

#[test]
fn paste_sanitizes_terminators_and_preserves_bracketed_newlines() {
    let modes = Modes {
        bracketed_paste: true,
        ..Modes::default()
    };
    assert_eq!(
        encode_paste("a\x1b[201~b\n界", &modes),
        "\x1b[200~ab\n界\x1b[201~".as_bytes()
    );
    assert_eq!(encode_paste("a\r\nb\nc", &Modes::default()), b"a\rb\rc");
}

#[test]
fn sgr_mouse_press_release_drag_wheel_and_modifiers() {
    let modes = Modes {
        mouse: MouseMode::Motion,
        sgr_mouse: true,
        ..Modes::default()
    };
    let cases = [
        (
            MouseButton::Left,
            MouseAction::Press,
            Mods::default(),
            "\x1b[<0;3;5M",
        ),
        (
            MouseButton::Right,
            MouseAction::Release,
            Mods::default(),
            "\x1b[<2;3;5m",
        ),
        (
            MouseButton::Left,
            MouseAction::Motion,
            Mods::default(),
            "\x1b[<32;3;5M",
        ),
        (
            MouseButton::NoButton,
            MouseAction::Motion,
            Mods::default(),
            "\x1b[<35;3;5M",
        ),
        (
            MouseButton::WheelUp,
            MouseAction::Press,
            Mods::default(),
            "\x1b[<64;3;5M",
        ),
        (
            MouseButton::WheelDown,
            MouseAction::Press,
            Mods::default(),
            "\x1b[<65;3;5M",
        ),
        (
            MouseButton::Middle,
            MouseAction::Press,
            Mods {
                ctrl: true,
                alt: true,
                shift: true,
                super_: false,
            },
            "\x1b[<29;3;5M",
        ),
    ];
    for (button, action, mods, expected) in cases {
        let input = MouseInput {
            point: CellPoint { col: 2, row: 4 },
            button,
            action,
            mods,
        };
        assert_eq!(encode_mouse(&input, &modes).unwrap(), expected.as_bytes());
    }
}

#[test]
fn mouse_reporting_modes_and_legacy_coordinates() {
    let mut input = MouseInput {
        point: CellPoint { col: 2, row: 4 },
        button: MouseButton::Left,
        action: MouseAction::Motion,
        mods: Mods::default(),
    };
    assert!(encode_mouse(&input, &Modes::default()).is_none());
    assert!(
        encode_mouse(
            &input,
            &Modes {
                mouse: MouseMode::Click,
                ..Modes::default()
            }
        )
        .is_none()
    );
    assert_eq!(
        encode_mouse(
            &input,
            &Modes {
                mouse: MouseMode::Drag,
                ..Modes::default()
            }
        )
        .unwrap(),
        [27, b'[', b'M', 64, 35, 37]
    );
    input.button = MouseButton::NoButton;
    assert!(
        encode_mouse(
            &input,
            &Modes {
                mouse: MouseMode::Drag,
                ..Modes::default()
            }
        )
        .is_none()
    );
    input.point.col = 223;
    assert!(
        encode_mouse(
            &input,
            &Modes {
                mouse: MouseMode::Motion,
                ..Modes::default()
            }
        )
        .is_none()
    );
}

#[test]
fn focus_reports_only_when_requested() {
    for focused in [true, false] {
        assert!(encode_focus(focused, &Modes::default()).is_none());
    }
    let modes = Modes {
        focus_events: true,
        ..Modes::default()
    };
    assert_eq!(encode_focus(true, &modes).unwrap(), b"\x1b[I");
    assert_eq!(encode_focus(false, &modes).unwrap(), b"\x1b[O");
}

#[test]
fn kitty_disambiguates_alt_characters() {
    let modes = Modes {
        kitty_flags: 1,
        ..Modes::default()
    };
    for (c, mods, expected) in [
        (
            'x',
            Mods {
                alt: true,
                ..Mods::default()
            },
            "\x1b[120;3u",
        ),
        (
            '[',
            Mods {
                alt: true,
                ..Mods::default()
            },
            "\x1b[91;3u",
        ),
        (
            'X',
            Mods {
                alt: true,
                shift: true,
                ..Mods::default()
            },
            "\x1b[120;4u",
        ),
        (
            'x',
            Mods {
                alt: true,
                ctrl: true,
                ..Mods::default()
            },
            "\x1b[120;7u",
        ),
    ] {
        assert_eq!(
            encode_key(&key(Key::Char(c), mods), &modes).unwrap(),
            expected.as_bytes()
        );
    }
}
