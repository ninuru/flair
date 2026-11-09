use super::*;
use crate::{
    command::{PaneRef, WindowRef},
    config::Config,
    geom::{Axis, Direction},
    key::{Locks, Mods},
};

fn key(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods: Mods::default(),
        text: Some(c.to_string()),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn leader() -> KeyInput {
    KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key('a')
    }
}

#[test]
fn prefix_runs_command_and_swallows_held_binding() {
    let mut machine = PrefixMachine::default();
    let config = KeysConfig::default();
    assert_eq!(
        machine.handle(&leader(), Millis(10), &config),
        PrefixResult::Consume
    );
    assert!(machine.pending());
    assert_eq!(
        machine.handle(&key('f'), Millis(11), &config),
        PrefixResult::Command(Command::Sessionizer)
    );
    assert!(!machine.pending());
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..key('f')
            },
            Millis(5000),
            &config
        ),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(&key('f'), Millis(5001), &config),
        PrefixResult::Forward
    );
}

#[test]
fn unknown_prefix_key_and_repeats_are_swallowed() {
    let mut machine = PrefixMachine::default();
    let config = KeysConfig::default();
    machine.handle(&leader(), Millis(0), &config);
    assert_eq!(
        machine.handle(&key('?'), Millis(1), &config),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..key('?')
            },
            Millis(2),
            &config
        ),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(&key('t'), Millis(3), &config),
        PrefixResult::Forward
    );
}

#[test]
fn timeout_generation_does_not_cancel_a_new_prefix() {
    let mut machine = PrefixMachine::default();
    let config = KeysConfig::default();
    machine.handle(&leader(), Millis(0), &config);
    let (old, until) = machine.deadline().unwrap();
    assert!(!machine.expire(old, Millis(1499)));
    assert!(machine.expire(old, until));
    machine.handle(&leader(), Millis(1501), &config);
    assert!(!machine.expire(old, Millis(3000)));
    assert!(machine.pending());
    assert_eq!(
        machine.handle(&key('f'), Millis(3001), &config),
        PrefixResult::Forward
    );
}

#[test]
fn ime_cancels_prefix_and_blocks_new_prefix() {
    let mut machine = PrefixMachine::default();
    let config = KeysConfig::default();
    machine.handle(&leader(), Millis(0), &config);
    machine.set_ime(true);
    assert!(!machine.pending());
    assert_eq!(
        machine.handle(&leader(), Millis(1), &config),
        PrefixResult::Consume
    );
    assert!(!machine.pending());
    machine.set_ime(false);
    assert_eq!(
        machine.handle(&key('f'), Millis(2), &config),
        PrefixResult::Forward
    );
}

#[test]
fn double_prefix_sends_literal_and_hold_does_not_arm() {
    let mut machine = PrefixMachine::default();
    let config = KeysConfig::default();
    machine.handle(&leader(), Millis(0), &config);
    assert_eq!(
        machine.handle(&leader(), Millis(1), &config),
        PrefixResult::SendPrefix
    );
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..leader()
            },
            Millis(2),
            &config
        ),
        PrefixResult::Consume
    );
    assert!(!machine.pending());
}

#[test]
fn all_default_bindings_dispatch_published_commands() {
    let config = KeysConfig::default();
    for (c, expected) in [
        ('c', Command::NewWindow),
        (',', Command::RenameWindow(None)),
        ('n', Command::NextWindow),
        ('p', Command::PreviousWindow),
        ('w', Command::ChooseWindow),
        ('d', Command::Detach),
        ('x', Command::ClosePane),
        ('&', Command::CloseWindow),
        ('z', Command::ToggleZoom),
        ('^', Command::LastWindow),
        ('l', Command::LastSession),
        ('f', Command::Sessionizer),
        ('s', Command::ChooseSession),
        ('r', Command::ReloadConfig),
        (
            'h',
            Command::SelectPane(PaneRef::Direction(Direction::Left)),
        ),
        (
            'j',
            Command::SelectPane(PaneRef::Direction(Direction::Down)),
        ),
        ('k', Command::SelectPane(PaneRef::Direction(Direction::Up))),
    ] {
        let mut machine = PrefixMachine::default();
        machine.handle(&leader(), Millis(0), &config);
        let input = KeyInput {
            mods: Mods {
                shift: c == '&' || c == '^',
                ..Mods::default()
            },
            ..key(c)
        };
        assert_eq!(
            machine.handle(&input, Millis(1), &config),
            PrefixResult::Command(expected),
            "{c}"
        );
    }
    for n in 1..=9 {
        let input = KeyInput {
            mods: Mods {
                alt: true,
                ..Mods::default()
            },
            ..key(char::from_digit(n, 10).unwrap())
        };
        assert_eq!(
            PrefixMachine::default().handle(&input, Millis(0), &config),
            PrefixResult::Command(Command::SelectWindow(WindowRef::Index(n as u16)))
        );
    }
    for (c, direction) in [
        ('h', Direction::Left),
        ('j', Direction::Down),
        ('k', Direction::Up),
        ('l', Direction::Right),
    ] {
        let input = KeyInput {
            mods: Mods {
                alt: true,
                ..Mods::default()
            },
            ..key(c)
        };
        assert_eq!(
            PrefixMachine::default().handle(&input, Millis(0), &config),
            PrefixResult::Command(Command::SelectPane(PaneRef::Direction(direction)))
        );
    }
    for (mods, axis) in [
        (
            Mods {
                ctrl: true,
                shift: true,
                ..Mods::default()
            },
            Axis::Horizontal,
        ),
        (
            Mods {
                alt: true,
                shift: true,
                ..Mods::default()
            },
            Axis::Vertical,
        ),
    ] {
        let input = KeyInput {
            key: Key::Named(NamedKey::Enter),
            mods,
            ..key('\r')
        };
        assert_eq!(
            PrefixMachine::default().handle(&input, Millis(0), &config),
            PrefixResult::Command(Command::Split(axis))
        );
    }
}

#[test]
fn configured_tables_override_defaults_and_unbound_chords_reach_apps() {
    let config = Config::from_toml("[keys]\nkey='C-b'\n[keys.prefix]\nf='last-session'\n[keys.root]\n'C-S-Enter'='none'\n'A-q'='new-window'").unwrap();
    let mut machine = PrefixMachine::default();
    assert_eq!(
        machine.handle(&leader(), Millis(0), &config.keys),
        PrefixResult::Forward
    );
    let prefix = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key('b')
    };
    assert_eq!(
        machine.handle(&prefix, Millis(1), &config.keys),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(&key('f'), Millis(2), &config.keys),
        PrefixResult::Command(Command::LastSession)
    );
    let split = KeyInput {
        key: Key::Named(NamedKey::Enter),
        mods: Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
        ..key('\r')
    };
    assert_eq!(
        machine.handle(&split, Millis(3), &config.keys),
        PrefixResult::Forward
    );
    let custom = KeyInput {
        mods: Mods {
            alt: true,
            ..Mods::default()
        },
        ..key('q')
    };
    assert_eq!(
        machine.handle(&custom, Millis(4), &config.keys),
        PrefixResult::Command(Command::NewWindow)
    );
    assert_eq!(
        machine.handle(
            &KeyInput {
                mods: Mods {
                    ctrl: true,
                    alt: true,
                    ..Mods::default()
                },
                ..key('x')
            },
            Millis(5),
            &config.keys
        ),
        PrefixResult::Forward
    );
}

#[test]
fn choose_session_binding_can_be_rebound_and_unbound() {
    let dispatch = |toml: &str, c: char| {
        let config = Config::from_toml(toml).unwrap();
        let mut machine = PrefixMachine::default();
        machine.handle(&leader(), Millis(0), &config.keys);
        machine.handle(&key(c), Millis(1), &config.keys)
    };
    assert_eq!(
        dispatch("", 's'),
        PrefixResult::Command(Command::ChooseSession)
    );
    assert_eq!(
        dispatch("", 'f'),
        PrefixResult::Command(Command::Sessionizer)
    );
    let rebound = "[keys.prefix]\ns='none'\ng='choose-session'";
    assert_eq!(dispatch(rebound, 's'), PrefixResult::Consume);
    assert_eq!(
        dispatch(rebound, 'g'),
        PrefixResult::Command(Command::ChooseSession)
    );
    assert_eq!(
        dispatch(rebound, 'f'),
        PrefixResult::Command(Command::Sessionizer)
    );
}

#[test]
fn release_of_consumed_key_is_swallowed_even_after_modifiers_change() {
    let config = KeysConfig::default();
    let mut machine = PrefixMachine::default();
    machine.handle(&leader(), Millis(0), &config);
    let punctuation = KeyInput {
        unshifted: Some('7'),
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
        ..key('&')
    };
    machine.handle(&punctuation, Millis(1), &config);
    let release = KeyInput {
        action: KeyAction::Release,
        unshifted: Some('7'),
        ..key('7')
    };
    assert_eq!(
        machine.handle(&release, Millis(2), &config),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(&key('7'), Millis(3), &config),
        PrefixResult::Forward
    );
}

#[test]
fn modifiers_and_unrelated_releases_do_not_spend_pending_prefix() {
    let config = KeysConfig::default();
    let mut machine = PrefixMachine::default();
    machine.handle(&leader(), Millis(0), &config);
    let modifier = KeyInput {
        key: Key::Named(NamedKey::ShiftLeft),
        ..key(' ')
    };
    assert_eq!(
        machine.handle(&modifier, Millis(1), &config),
        PrefixResult::Forward
    );
    let release = KeyInput {
        action: KeyAction::Release,
        ..key('z')
    };
    assert_eq!(
        machine.handle(&release, Millis(2), &config),
        PrefixResult::Forward
    );
    assert!(machine.pending());
    assert_eq!(
        machine.handle(
            &KeyInput {
                mods: Mods {
                    shift: true,
                    ..Mods::default()
                },
                ..key('&')
            },
            Millis(3),
            &config
        ),
        PrefixResult::Command(Command::CloseWindow)
    );
}

#[test]
fn reload_resets_pending_generation_and_replaces_bindings_without_leaking_holds() {
    let mut machine = PrefixMachine::default();
    let old = KeysConfig::default();
    machine.handle(&leader(), Millis(0), &old);
    let (generation, _) = machine.deadline().unwrap();
    machine.handle(&key('f'), Millis(1), &old);
    let new =
        Config::from_toml("[keys]\nkey='C-b'\ntimeout_ms=20\n[keys.prefix]\nf='reload-config'")
            .unwrap();
    machine.reset();
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..key('f')
            },
            Millis(2),
            &new.keys
        ),
        PrefixResult::Consume
    );
    let prefix = KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key('b')
    };
    machine.handle(&prefix, Millis(3), &new.keys);
    assert!(!machine.expire(generation, Millis(4)));
    assert_eq!(
        machine.handle(&key('f'), Millis(5), &new.keys),
        PrefixResult::Command(Command::ReloadConfig)
    );
    machine.handle(&prefix, Millis(6), &new.keys);
    assert_eq!(
        machine.handle(&key('f'), Millis(26), &new.keys),
        PrefixResult::Forward
    );
}

#[test]
fn repeat_of_a_previously_held_application_key_does_not_execute_prefix_binding() {
    let config = KeysConfig::default();
    let mut machine = PrefixMachine::default();
    machine.handle(&key('f'), Millis(0), &config);
    machine.handle(&leader(), Millis(1), &config);
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..key('f')
            },
            Millis(2),
            &config
        ),
        PrefixResult::Consume
    );
    assert!(!machine.pending());
    assert_eq!(
        machine.handle(
            &KeyInput {
                action: KeyAction::Release,
                ..key('f')
            },
            Millis(3),
            &config
        ),
        PrefixResult::Forward
    );
}

#[test]
fn ime_consumed_press_and_held_repeats_never_leak_release() {
    let config = KeysConfig::default();
    let mut machine = PrefixMachine::default();
    machine.set_ime(true);
    assert_eq!(
        machine.handle(&key('x'), Millis(0), &config),
        PrefixResult::Consume
    );
    machine.set_ime(false);
    assert_eq!(
        machine.handle(
            &KeyInput {
                repeat: true,
                ..key('x')
            },
            Millis(1),
            &config
        ),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(
            &KeyInput {
                action: KeyAction::Release,
                ..key('x')
            },
            Millis(2),
            &config
        ),
        PrefixResult::Consume
    );
    assert_eq!(
        machine.handle(&key('x'), Millis(3), &config),
        PrefixResult::Forward
    );
}

#[test]
fn ime_does_not_suppress_release_of_a_previously_forwarded_press() {
    let config = KeysConfig::default();
    let mut machine = PrefixMachine::default();
    machine.handle(&key('x'), Millis(0), &config);
    machine.set_ime(true);
    machine.handle(
        &KeyInput {
            repeat: true,
            ..key('x')
        },
        Millis(1),
        &config,
    );
    assert_eq!(
        machine.handle(
            &KeyInput {
                action: KeyAction::Release,
                ..key('x')
            },
            Millis(2),
            &config
        ),
        PrefixResult::Forward
    );
}

#[test]
fn clipboard_and_scroll_shortcuts_respect_prefix_and_configured_tables() {
    let config = KeysConfig::default();
    let paste = KeyInput {
        mods: Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
        ..key('V')
    };
    let mut machine = PrefixMachine::default();
    assert_eq!(
        machine.handle(&paste, Millis(0), &config),
        PrefixResult::Command(Command::PasteClipboard)
    );
    machine.handle(&leader(), Millis(1), &config);
    assert_eq!(
        machine.handle(&paste, Millis(2), &config),
        PrefixResult::Consume
    );
    let custom = Config::from_toml("[keys.root]\n'C-S-v'='new-window'\n'S-PageUp'='none'").unwrap();
    assert_eq!(
        machine.handle(&paste, Millis(3), &custom.keys),
        PrefixResult::Command(Command::NewWindow)
    );
    let scroll = KeyInput {
        key: Key::Named(NamedKey::PageUp),
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
        ..key(' ')
    };
    assert_eq!(
        machine.handle(&scroll, Millis(4), &custom.keys),
        PrefixResult::Forward
    );
    assert_eq!(
        PrefixMachine::default().handle(&scroll, Millis(4), &config),
        PrefixResult::Command(Command::ScrollPages(1))
    );
}
