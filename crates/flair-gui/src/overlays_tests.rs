use flair_core::{
    command::WindowRef,
    key::{Locks, Mods},
    proto::Target,
};

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

fn sessions() -> ServerMsg {
    ServerMsg::ChooseSession {
        sessions: ["main", "notes", "work"]
            .into_iter()
            .map(|name| flair_core::proto::SessionInfo {
                name: name.into(),
                root: format!("/srv/{name}").into(),
                windows: 1,
            })
            .collect(),
        current: "main".into(),
    }
}

fn hits(rows: usize) -> OverlayHits {
    let rect = |y: f32, h: f32| crate::renderer::PxRect {
        x: 100.0,
        y,
        w: 200.0,
        h,
    };
    OverlayHits {
        panel: rect(100.0, 400.0),
        rows: (0..rows)
            .map(|index| (index, rect(150.0 + 40.0 * index as f32, 32.0)))
            .collect(),
        controls: Vec::new(),
    }
}

fn windows() -> StatusLine {
    StatusLine {
        windows: (1..=3)
            .map(|index| flair_core::status::StatusWindow {
                id: flair_core::ids::WindowId(u32::from(index) + 10),
                index,
                name: format!("w{index}"),
                active: index == 1,
                last: false,
            })
            .collect(),
        ..StatusLine::default()
    }
}

#[test]
fn hit_test_uses_the_last_layout_only_while_a_panel_is_open() {
    let mut overlays = Overlays::default();
    overlays.place(Some(hits(2)));
    assert_eq!(overlays.hit_test((150.0, 160.0)), None);
    open(&mut overlays, sessions());
    assert_eq!(overlays.hit_test((150.0, 160.0)), Some(OverlayHit::Row(0)));
    assert_eq!(overlays.hit_test((150.0, 120.0)), Some(OverlayHit::Panel));
    assert_eq!(overlays.hit_test((5.0, 5.0)), Some(OverlayHit::Outside));
    let mut prompt = Overlays::default();
    open(&mut prompt, ServerMsg::PromptSearch { forward: true });
    prompt.place(Some(hits(2)));
    assert_eq!(prompt.hit_test((150.0, 160.0)), None);
}

#[test]
fn clicking_a_session_row_switches_to_it() {
    let mut overlays = Overlays::default();
    open(&mut overlays, sessions());
    assert_eq!(overlays.click(OverlayHit::Panel), OverlayEvent::Ignored);
    assert_eq!(overlays.click(OverlayHit::Row(7)), OverlayEvent::Ignored);
    assert!(overlays.is_open());
    let OverlayEvent::Send(ClientMsg::Command(Command::Switch(target))) =
        overlays.click(OverlayHit::Row(1))
    else {
        panic!("row click did not switch");
    };
    assert_eq!(target, Target::Name("work".into()));
    assert!(!overlays.is_open());
}

#[test]
fn clicking_outside_closes_pickers_and_reverts_the_theme() {
    let mut overlays = Overlays::default();
    open(&mut overlays, sessions());
    assert_eq!(overlays.click(OverlayHit::Outside), OverlayEvent::Redraw);
    assert!(!overlays.is_open());
    overlays.open(
        &ServerMsg::ChooseWindow,
        &windows(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    );
    assert_eq!(overlays.click(OverlayHit::Outside), OverlayEvent::Redraw);
    assert!(!overlays.is_open());
    open(&mut overlays, ServerMsg::ChooseTheme);
    assert_eq!(
        overlays.click(OverlayHit::Outside),
        OverlayEvent::Theme(ThemeOutcome::Cancel)
    );
    assert!(!overlays.is_open());
}

#[test]
fn clicking_a_window_row_selects_that_window() {
    let mut overlays = Overlays::default();
    overlays.open(
        &ServerMsg::ChooseWindow,
        &windows(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    );
    assert_eq!(
        overlays.click(OverlayHit::Row(2)),
        OverlayEvent::Send(ClientMsg::Command(Command::SelectWindow(WindowRef::Id(
            flair_core::ids::WindowId(13)
        ))))
    );
    assert!(!overlays.is_open());
}

#[test]
fn clicking_a_theme_row_previews_it_and_keeps_the_picker_open() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::ChooseTheme);
    let OverlayEvent::Theme(ThemeOutcome::Preview(name)) = overlays.click(OverlayHit::Row(0))
    else {
        panic!("row click did not preview");
    };
    assert!(overlays.is_open());
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Enter)), || None),
        Some(OverlayEvent::Theme(ThemeOutcome::Save(name)))
    );
}

#[test]
fn wheel_and_page_keys_move_the_selection() {
    let mut overlays = Overlays::default();
    overlays.open(
        &ServerMsg::ChooseWindow,
        &windows(),
        &Config::default(),
        Path::new("/nonexistent/flair.toml"),
    );
    let selected = |overlays: &Overlays| match &overlays.prompt {
        Some(Prompt::Choose { selected, .. }) => *selected,
        Some(Prompt::Rename { .. } | Prompt::Search { .. }) | None => usize::MAX,
    };
    assert_eq!(overlays.wheel(1), OverlayEvent::Redraw);
    assert_eq!(selected(&overlays), 1);
    overlays.place(Some(hits(2)));
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::PageDown)), || None),
        Some(OverlayEvent::Redraw)
    );
    assert_eq!(selected(&overlays), 2);
    overlays.key(&press(Key::Named(NamedKey::PageUp)), || None);
    assert_eq!(selected(&overlays), 0);
    open(&mut overlays, sessions());
    overlays.prompt = None;
    overlays.wheel(2);
    let picker = overlays.picker.as_ref().unwrap();
    assert_eq!(picker.accept(), Some(Target::Name("notes".into())));
    let mut idle = Overlays::default();
    assert_eq!(idle.wheel(1), OverlayEvent::Ignored);
}

fn release(key: Key) -> KeyInput {
    KeyInput {
        action: KeyAction::Release,
        ..press(key)
    }
}

#[test]
fn a_notice_stays_until_the_next_key_press() {
    let mut overlays = Overlays::default();
    assert_eq!(overlays.notice(), None);
    overlays.set_notice(Notice::info("theme saved: nord"));
    assert_eq!(overlays.notice(), Some(&Notice::info("theme saved: nord")));
    assert_eq!(
        overlays.key(&release(Key::Named(NamedKey::Enter)), || None),
        None
    );
    assert_eq!(overlays.notice(), Some(&Notice::info("theme saved: nord")));
    assert_eq!(overlays.key(&press(Key::Char('a')), || None), None);
    assert_eq!(overlays.notice(), None);
}

#[test]
fn clear_notice_drops_it() {
    let mut overlays = Overlays::default();
    overlays.set_notice(Notice::error("config not reloaded: bad"));
    overlays.clear_notice();
    assert_eq!(overlays.notice(), None);
}

#[test]
fn opening_an_overlay_clears_the_notice() {
    let mut overlays = Overlays::default();
    overlays.set_notice(Notice::info("config reloaded"));
    open(&mut overlays, sessions());
    assert_eq!(overlays.notice(), None);
}

#[test]
fn an_empty_rename_shows_an_error_notice_while_the_prompt_stays_open() {
    let mut overlays = Overlays::default();
    open(
        &mut overlays,
        ServerMsg::PromptRename {
            current: "x".to_owned(),
        },
    );
    overlays.key(&press(Key::Named(NamedKey::Backspace)), || None);
    overlays.key(&press(Key::Named(NamedKey::Enter)), || None);
    assert!(overlays.is_open());
    assert_eq!(
        overlays.notice(),
        Some(&Notice::error("window name cannot be empty"))
    );
    overlays.key(&release(Key::Named(NamedKey::Enter)), || None);
    assert!(overlays.notice().is_some());
    overlays.key(&press(Key::Char('y')), || None);
    assert_eq!(overlays.notice(), None);
}

#[test]
fn escape_closes_a_session_picker_without_sending_anything() {
    let mut overlays = Overlays::default();
    open(&mut overlays, sessions());
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Escape)), || None),
        Some(OverlayEvent::Redraw)
    );
    assert!(!overlays.is_open());
}
