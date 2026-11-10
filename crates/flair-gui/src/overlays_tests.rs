use flair_core::{
    command::WindowRef,
    key::{Locks, Mods},
    proto::Target,
};
use std::fs;

use super::*;
use crate::settings_layout::{Entry, SettingsHit, SettingsSheet};

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
        prompt_scroll_limit: None,
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

fn settings_page(overlays: &mut Overlays) -> SettingsSheet {
    overlays.settings_mut().unwrap().sheet()
}

#[test]
fn the_settings_message_opens_the_page() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    assert!(overlays.is_open());
    assert!(overlays.panel_open());
    assert_eq!(settings_page(&mut overlays).crumb, "Appearance");
}

#[test]
fn the_settings_page_reads_sources_from_the_config_file() {
    let dir = std::env::temp_dir().join(format!("flair-overlays-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("flair.toml");
    fs::write(&path, "[font]\nsize = 14\n").unwrap();
    let mut overlays = Overlays::default();
    let config = Config::from_toml("[font]\nsize = 14\n").unwrap();
    assert!(overlays.open(&ServerMsg::Settings, &StatusLine::default(), &config, &path));
    fs::remove_dir_all(&dir).unwrap();
    let page = overlays.settings_mut().unwrap();
    page.click(SettingsHit::Section(1));
    page.focus(1);
    let sheet = page.sheet();
    let sources: Vec<_> = sheet.groups[0]
        .entries
        .iter()
        .map(|entry| match entry {
            Entry::Setting { source, .. } | Entry::Binding { source, .. } => *source,
        })
        .collect();
    assert_eq!(
        sources,
        ["default", "flair.toml", "default", "default", "default"]
    );
}

#[test]
fn keys_and_text_go_to_the_settings_page() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Down)), || None),
        Some(OverlayEvent::Settings(SettingsOutcome::Pending))
    );
    assert_eq!(settings_page(&mut overlays).focus, Some(1));
    assert_eq!(
        overlays.text("pad"),
        Some(OverlayEvent::Settings(SettingsOutcome::Pending))
    );
    assert_eq!(settings_page(&mut overlays).search.text, "pad");
}

#[test]
fn page_keys_scroll_the_settings_page_by_the_drawn_rows() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    overlays.place(Some(hits(3)));
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::PageDown)), || None),
        Some(OverlayEvent::Settings(SettingsOutcome::Pending))
    );
    assert_eq!(settings_page(&mut overlays).focus, Some(3));
    overlays.key(&press(Key::Named(NamedKey::PageUp)), || None);
    assert_eq!(settings_page(&mut overlays).focus, Some(0));
    assert_eq!(
        overlays.wheel(2),
        OverlayEvent::Settings(SettingsOutcome::Pending)
    );
    assert_eq!(settings_page(&mut overlays).focus, Some(2));
}

#[test]
fn the_settings_page_ignores_clicks_on_the_page_and_the_bar() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    assert_eq!(overlays.click(OverlayHit::Outside), OverlayEvent::Ignored);
    assert_eq!(overlays.click(OverlayHit::Panel), OverlayEvent::Ignored);
    assert!(overlays.is_open());
}

#[test]
fn closing_the_settings_page_drops_it() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    assert_eq!(
        overlays.key(&press(Key::Named(NamedKey::Escape)), || None),
        Some(OverlayEvent::Settings(SettingsOutcome::Close))
    );
    assert!(!overlays.is_open());
    assert!(overlays.settings_mut().is_none());
    open(&mut overlays, ServerMsg::Settings);
    assert_eq!(
        overlays.click(OverlayHit::Settings(SettingsHit::Close)),
        OverlayEvent::Settings(SettingsOutcome::Close)
    );
    assert!(!overlays.is_open());
}

#[test]
fn settings_hits_and_rows_route_to_the_page() {
    let mut overlays = Overlays::default();
    assert_eq!(
        overlays.click(OverlayHit::Settings(SettingsHit::Search)),
        OverlayEvent::Ignored
    );
    open(&mut overlays, ServerMsg::Settings);
    assert_eq!(
        overlays.click(OverlayHit::Row(2)),
        OverlayEvent::Settings(SettingsOutcome::Pending)
    );
    assert_eq!(settings_page(&mut overlays).focus, Some(2));
    assert_eq!(
        overlays.click(OverlayHit::Settings(SettingsHit::Section(5))),
        OverlayEvent::Settings(SettingsOutcome::Pending)
    );
    assert_eq!(settings_page(&mut overlays).crumb, "Keys");
}

#[test]
fn paste_goes_to_the_settings_editor() {
    let mut overlays = Overlays::default();
    open(&mut overlays, ServerMsg::Settings);
    overlays.click(OverlayHit::Row(1));
    overlays.key(&press(Key::Named(NamedKey::Enter)), || None);
    let expected = Config::from_toml("[colors]\nbackground = \"#ff0000\"\n").unwrap();
    assert_eq!(
        overlays.key(&paste(), || Some("#ff0000".to_owned())),
        Some(OverlayEvent::Settings(SettingsOutcome::Preview(Box::new(
            expected
        ))))
    );
    assert_eq!(overlays.key(&paste(), || None), Some(OverlayEvent::Ignored));
}

#[test]
fn a_preview_resolves_the_theme_of_its_config() {
    let config = Config::default().with_theme("gruvbox-dark");
    let preview = Preview::resolve(config.clone(), Path::new("/nonexistent/flair.toml"));
    assert_eq!(preview.theme, config.theme());
    assert_eq!(preview.config, config);
}

#[test]
fn an_unreadable_config_shows_an_error_instead_of_the_settings_page() {
    let dir = std::env::temp_dir().join(format!("flair-overlays-dir-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut overlays = Overlays::default();
    let opened = overlays.open(
        &ServerMsg::Settings,
        &StatusLine::default(),
        &Config::default(),
        &dir,
    );
    fs::remove_dir_all(&dir).unwrap();
    assert!(opened);
    assert!(!overlays.is_open());
    let notice = overlays.notice().unwrap();
    assert!(notice.error);
    assert!(notice.text.starts_with("settings: "), "{}", notice.text);
}
