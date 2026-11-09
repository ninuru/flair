use flair_core::key::{Locks, Mods};

use super::*;

fn press(key: Key) -> KeyInput {
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
fn choose_window_opens_a_prompt_and_escape_closes_it() {
    let mut overlays = Overlays::default();
    assert!(overlays.open(
        &ServerMsg::ChooseWindow,
        &StatusLine::default(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    ));
    assert!(overlays.is_open());
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Escape)), || None),
        Some(OverlayEvent::Redraw)
    );
    assert!(!overlays.is_open());
}

#[test]
fn keys_and_text_pass_through_when_nothing_is_open() {
    let mut overlays = Overlays::default();
    assert!(!overlays.open(
        &ServerMsg::Bell,
        &StatusLine::default(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    ));
    assert_eq!(overlays.key(&press(Key::Char('a')), || None), None);
    assert_eq!(overlays.text("a"), None);
}

#[test]
fn theme_picker_closes_on_cancel() {
    let mut overlays = Overlays::default();
    overlays.open(
        &ServerMsg::ChooseTheme,
        &StatusLine::default(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    );
    assert!(overlays.theme_picker_mut().is_some());
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Escape)), || None),
        Some(OverlayEvent::Theme(ThemeOutcome::Cancel))
    );
    assert!(!overlays.is_open());
}

fn open(overlays: &mut Overlays, message: ServerMsg) {
    assert!(overlays.open(
        &message,
        &StatusLine::default(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    ));
}

fn paste() -> KeyInput {
    KeyInput {
        mods: Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        },
        ..press(Key::Char('V'))
    }
}

#[test]
fn paste_inserts_the_clipboard_into_a_search_prompt() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::PromptSearch { forward: true });
    assert_eq!(
        overlays.key(&paste(), || Some("grüße\n".to_owned())),
        Some(OverlayEvent::Redraw)
    );
    assert_eq!(overlays.bar_text().as_deref(), Some("/grüße"));
    assert_eq!(overlays.key(&paste(), || None), Some(OverlayEvent::Ignored));
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Enter)), || None),
        Some(OverlayEvent::Send(ClientMsg::Command(Command::Copy(
            flair_core::command::CopyCommand::Search {
                pattern: "grüße".to_owned(),
                forward: true,
            }
        ))))
    );
    assert!(!overlays.is_open());
}

#[test]
fn empty_rename_keeps_the_prompt_open() {
    let mut overlays = Overlays::default();
    open(
        &mut overlays,
        ServerMsg::PromptRename {
            current: "x".to_owned(),
        },
    );
    overlays.key(&press(Key::Named(NamedKey::Backspace)), || None);
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Enter)), || None),
        Some(OverlayEvent::Redraw)
    );
    assert!(overlays.is_open());
    assert_eq!(overlays.bar_text().as_deref(), Some("(rename-window) "));
}

#[test]
fn picker_query_edits_mid_text() {
    let mut overlays = Overlays::default();
    open(
        &mut overlays,
        ServerMsg::ChooseSession {
            sessions: Vec::new(),
            current: "main".to_owned(),
        },
    );
    overlays.text("flr");
    overlays.key(&press(Key::Named(NamedKey::Left)), || None);
    overlays.text("a");
    overlays.key(&paste(), || Some("i".to_owned()));
    let picker = overlays.picker.as_ref().unwrap();
    assert_eq!(picker.query.text(), "flair");
    assert_eq!(picker.query.cursor(), 4);
}
