use super::*;
use flair_core::key::{Key, KeyAction, Locks, Mods, NamedKey};

fn key(key: Key, text: Option<&str>) -> KeyInput {
    KeyInput {
        key,
        mods: Mods::default(),
        text: text.map(str::to_owned),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn ctrl(c: char) -> KeyInput {
    KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key(Key::Char(c), None)
    }
}

fn named(name: NamedKey) -> KeyInput {
    key(Key::Named(name), None)
}

fn shift_tab() -> KeyInput {
    KeyInput {
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
        ..named(NamedKey::Tab)
    }
}

fn typed(c: char) -> KeyInput {
    key(Key::Char(c), Some(&c.to_string()))
}

fn names() -> Vec<String> {
    ["catppuccin-mocha", "flair-dark", "gruvbox-dark"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn page(text: &str) -> SettingsPage {
    let config = Config::from_toml(text).unwrap();
    let theme = config.theme();
    SettingsPage::new(&config, theme, text.to_owned(), names())
}

fn press(page: &mut SettingsPage, name: NamedKey) -> SettingsOutcome {
    page.key(&named(name))
}

fn type_text(page: &mut SettingsPage, text: &str) -> SettingsOutcome {
    text.chars()
        .map(|c| page.key(&typed(c)))
        .last()
        .unwrap_or(SettingsOutcome::Pending)
}

fn section(page: &mut SettingsPage, index: usize) {
    assert_eq!(
        page.click(SettingsHit::Section(index)),
        SettingsOutcome::Pending
    );
}

fn focus_key(page: &mut SettingsPage, key: &str) {
    let section_index = Section::ALL
        .iter()
        .position(|section| *section == SETTINGS[position(key).unwrap()].section)
        .unwrap();
    section(page, section_index);
    let offset = SETTINGS
        .iter()
        .filter(|setting| setting.section == Section::ALL[section_index])
        .position(|setting| setting.key == key)
        .unwrap();
    page.focus(offset);
}

fn entries(sheet: &SettingsSheet) -> Vec<&Entry> {
    sheet
        .groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .collect()
}

fn titles(sheet: &SettingsSheet) -> Vec<&'static str> {
    entries(sheet)
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Setting { title, .. } => Some(*title),
            Entry::Binding { .. } => None,
        })
        .collect()
}

fn focused(page: &SettingsPage) -> Entry {
    let sheet = page.sheet();
    entries(&sheet)[sheet.focus.unwrap()].clone()
}

fn control(page: &SettingsPage) -> Control {
    match focused(page) {
        Entry::Setting { control, .. } => control,
        Entry::Binding { .. } => panic!("binding focused"),
    }
}

fn error(page: &SettingsPage) -> Option<String> {
    match focused(page) {
        Entry::Setting { error, .. } => error,
        Entry::Binding { .. } => None,
    }
}

fn source(page: &SettingsPage) -> &'static str {
    match focused(page) {
        Entry::Setting { source, .. } | Entry::Binding { source, .. } => source,
    }
}

fn save(key: &'static str, value: Option<Value>) -> SettingsOutcome {
    SettingsOutcome::Save { key, value }
}

fn preview_of(text: &str, key: &str, value: Option<&Value>) -> SettingsOutcome {
    SettingsOutcome::Preview(Box::new(apply(text, key, value).unwrap().1))
}

fn default_keys() -> Vec<(&'static str, String)> {
    [
        (ENTER_KEY, "edit"),
        (SPACE_KEY, "toggle"),
        (STEP_KEY, "step"),
        (DELETE_KEY, "reset"),
        (ESCAPE_KEY, "close"),
        (SEARCH_KEY, "search"),
    ]
    .into_iter()
    .map(|(cap, label)| (cap, label.to_owned()))
    .collect()
}

fn editing_keys() -> Vec<(&'static str, String)> {
    vec![
        (ENTER_KEY, "save".to_owned()),
        (ESCAPE_KEY, "revert".to_owned()),
    ]
}

fn pending_keys() -> Vec<(&'static str, String)> {
    let mut keys = editing_keys();
    keys.push((STEP_KEY, "step".to_owned()));
    keys
}

#[test]
fn a_new_page_shows_appearance_with_the_first_entry_focused() {
    let page = page("");
    let sheet = page.sheet();
    assert_eq!(
        sheet.sections,
        ["Appearance", "Font", "Window", "Terminal", "Keys"]
    );
    assert_eq!(sheet.section, Some(0));
    assert!(!sheet.sections_focused);
    assert_eq!(sheet.crumb, "Appearance");
    assert_eq!(sheet.search.text, "");
    assert_eq!(sheet.search.placeholder, "Search settings");
    assert_eq!(sheet.groups.len(), 1);
    assert_eq!(sheet.groups[0].heading, None);
    assert_eq!(
        titles(&sheet),
        [
            "Theme",
            "Background",
            "Foreground",
            "Cursor colour",
            "Accent",
            "Selection"
        ]
    );
    assert_eq!(sheet.focus, Some(0));
    assert_eq!(sheet.keys, default_keys());
    assert_eq!(control(&page), Control::Choice("flair-dark".to_owned()));
}

#[test]
fn entries_show_values_and_their_source() {
    let mut page = page("[font]\nsize = 14\n");
    section(&mut page, 1);
    page.focus(1);
    assert_eq!(control(&page), Control::Number("14".to_owned()));
    assert_eq!(source(&page), "flair.toml");
    page.focus(2);
    assert_eq!(control(&page), Control::Number("1".to_owned()));
    assert_eq!(source(&page), "default");
    section(&mut page, 4);
    assert_eq!(control(&page), Control::Toggle(true));
    page.focus(1);
    assert_eq!(control(&page), Control::Text("C-a".to_owned()));
}

#[test]
fn unset_colours_show_the_theme_value_as_default() {
    let mut page = page("");
    page.focus(1);
    let theme = Config::default().theme();
    assert_eq!(control(&page), Control::Text(theme.background.hex()));
    assert_eq!(source(&page), "default");
}

#[test]
fn the_keys_section_lists_settings_then_key_bindings() {
    let mut page = page("[keys.prefix]\nc = \"none\"\n");
    section(&mut page, 4);
    let sheet = page.sheet();
    assert_eq!(sheet.crumb, "Keys");
    assert_eq!(sheet.groups.len(), 2);
    assert_eq!(sheet.groups[0].heading, None);
    assert_eq!(titles(&sheet), ["Bindings", "Prefix key", "Prefix timeout"]);
    assert_eq!(sheet.groups[1].heading, Some("Key bindings"));
    assert_eq!(
        sheet.groups[1].entries.len(),
        Config::default().keys.bindings().len()
    );
    assert_eq!(
        sheet.groups[1].entries[0],
        Entry::Binding {
            chord: "c".to_owned(),
            command: "unbound".to_owned(),
            unbound: true,
            table: "prefix",
            source: "flair.toml",
        }
    );
    assert_eq!(
        sheet.groups[1].entries[1],
        Entry::Binding {
            chord: ",".to_owned(),
            command: "rename-window".to_owned(),
            unbound: false,
            table: "prefix",
            source: "default",
        }
    );
}

#[test]
fn up_and_down_move_the_focus_and_clamp() {
    let mut page = page("");
    assert_eq!(press(&mut page, NamedKey::Up), SettingsOutcome::Pending);
    assert_eq!(page.sheet().focus, Some(0));
    assert_eq!(press(&mut page, NamedKey::Down), SettingsOutcome::Pending);
    assert_eq!(page.key(&ctrl('n')), SettingsOutcome::Pending);
    assert_eq!(page.sheet().focus, Some(2));
    page.key(&ctrl('p'));
    assert_eq!(page.sheet().focus, Some(1));
    for _ in 0..10 {
        press(&mut page, NamedKey::Down);
    }
    assert_eq!(page.sheet().focus, Some(5));
}

#[test]
fn releases_do_nothing() {
    let mut page = page("");
    let release = KeyInput {
        action: KeyAction::Release,
        ..named(NamedKey::Escape)
    };
    assert_eq!(page.key(&release), SettingsOutcome::Pending);
    assert_eq!(page.sheet().focus, Some(0));
}

#[test]
fn tab_moves_between_the_columns() {
    let mut page = page("");
    press(&mut page, NamedKey::Down);
    assert_eq!(press(&mut page, NamedKey::Tab), SettingsOutcome::Pending);
    assert!(page.sheet().sections_focused);
    assert_eq!(press(&mut page, NamedKey::Down), SettingsOutcome::Pending);
    let sheet = page.sheet();
    assert_eq!(sheet.section, Some(1));
    assert_eq!(sheet.crumb, "Font");
    assert_eq!(sheet.focus, Some(0));
    press(&mut page, NamedKey::Down);
    press(&mut page, NamedKey::Up);
    assert_eq!(page.sheet().section, Some(1));
    page.key(&shift_tab());
    assert!(!page.sheet().sections_focused);
    page.key(&shift_tab());
    assert!(page.sheet().sections_focused);
    press(&mut page, NamedKey::Right);
    assert!(!page.sheet().sections_focused);
    press(&mut page, NamedKey::Tab);
    press(&mut page, NamedKey::Enter);
    assert!(!page.sheet().sections_focused);
    press(&mut page, NamedKey::Tab);
    press(&mut page, NamedKey::Up);
    press(&mut page, NamedKey::Up);
    assert_eq!(page.sheet().section, Some(0));
    for _ in 0..10 {
        press(&mut page, NamedKey::Down);
    }
    assert_eq!(page.sheet().section, Some(4));
}

#[test]
fn escape_closes_from_either_column() {
    let mut page = page("");
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Close);
    press(&mut page, NamedKey::Tab);
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Close);
}

#[test]
fn space_and_enter_flip_a_toggle_and_save_at_once() {
    let mut page = page("");
    focus_key(&mut page, "keys.enabled");
    assert_eq!(
        page.key(&typed(' ')),
        save("keys.enabled", Some(Value::Bool(false)))
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("keys.enabled", Some(Value::Bool(false)))
    );
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn space_does_nothing_on_other_kinds() {
    let mut page = page("");
    assert_eq!(page.key(&typed(' ')), SettingsOutcome::Pending);
    assert_eq!(page.sheet().search.text, "");
}

#[test]
fn stepping_a_number_previews_a_pending_value_and_enter_saves_it() {
    let mut page = page("");
    focus_key(&mut page, "font.size");
    let value = Value::Float(13.5);
    assert_eq!(
        press(&mut page, NamedKey::Right),
        preview_of("", "font.size", Some(&value))
    );
    assert_eq!(control(&page), Control::Number("13.5".to_owned()));
    assert_eq!(page.sheet().keys, pending_keys());
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("font.size", Some(value))
    );
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn escape_reverts_a_pending_value_then_closes() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Left);
    assert_eq!(control(&page), Control::Number("7".to_owned()));
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Revert);
    assert_eq!(control(&page), Control::Number("8".to_owned()));
    assert_eq!(page.sheet().keys, default_keys());
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Close);
}

#[test]
fn stepping_back_to_the_saved_value_reverts() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Right);
    assert_eq!(press(&mut page, NamedKey::Left), SettingsOutcome::Revert);
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn a_clamped_step_changes_nothing() {
    let mut page = page("[window]\npadding_y = 0\n");
    focus_key(&mut page, "window.padding_y");
    assert_eq!(press(&mut page, NamedKey::Left), SettingsOutcome::Pending);
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn leaving_a_pending_value_saves_it_and_still_moves() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Right);
    assert_eq!(
        press(&mut page, NamedKey::Down),
        save("window.padding_x", Some(Value::Integer(9)))
    );
    assert_eq!(page.sheet().focus, Some(1));
    assert_eq!(page.sheet().keys, default_keys());
    press(&mut page, NamedKey::Right);
    assert_eq!(
        press(&mut page, NamedKey::Tab),
        save("window.padding_y", Some(Value::Integer(1)))
    );
    assert!(page.sheet().sections_focused);
}

#[test]
fn pending_is_saved_before_a_query_change() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.key(&typed('f')),
        save("window.padding_x", Some(Value::Integer(9)))
    );
    assert_eq!(page.sheet().search.text, "f");
}

#[test]
fn the_theme_cycles_as_a_pending_preview() {
    let mut page = page("");
    let next = Value::Text("gruvbox-dark".to_owned());
    assert_eq!(
        press(&mut page, NamedKey::Right),
        preview_of("", "theme", Some(&next))
    );
    assert_eq!(control(&page), Control::Choice("gruvbox-dark".to_owned()));
    press(&mut page, NamedKey::Right);
    assert_eq!(
        control(&page),
        Control::Choice("catppuccin-mocha".to_owned())
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("theme", Some(Value::Text("catppuccin-mocha".to_owned())))
    );
}

#[test]
fn enter_on_a_choice_cycles_forward_as_pending() {
    let mut page = page("");
    let next = Value::Text("gruvbox-dark".to_owned());
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        preview_of("", "theme", Some(&next))
    );
    assert_eq!(page.sheet().keys, pending_keys());
}

#[test]
fn enter_opens_an_editor_seeded_with_the_value() {
    let mut page = page("");
    focus_key(&mut page, "font.size");
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Pending);
    assert_eq!(
        control(&page),
        Control::Editor(Field {
            text: "13".to_owned(),
            cursor: 2,
            placeholder: "unset",
        })
    );
    assert_eq!(page.sheet().keys, editing_keys());
}

#[test]
fn typing_a_valid_value_previews_and_an_invalid_one_waits() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    let two = Value::Integer(2);
    assert_eq!(
        page.key(&typed('2')),
        preview_of("", "window.padding_x", Some(&two))
    );
    assert_eq!(page.key(&typed('x')), SettingsOutcome::Pending);
    assert_eq!(press(&mut page, NamedKey::Left), SettingsOutcome::Pending);
    assert_eq!(
        press(&mut page, NamedKey::Delete),
        preview_of("", "window.padding_x", Some(&two))
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("window.padding_x", Some(two))
    );
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn font_families_never_preview_while_typing() {
    let mut page = page("");
    focus_key(&mut page, "font.family");
    press(&mut page, NamedKey::Enter);
    assert_eq!(page.key(&typed('X')), SettingsOutcome::Pending);
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save(
            "font.family",
            Some(Value::Text("JetBrainsMono Nerd Font MonoX".to_owned()))
        )
    );
}

#[test]
fn escape_in_the_editor_reverts_then_closes() {
    let mut page = page("");
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    type_text(&mut page, "x");
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Revert);
    assert_eq!(control(&page), Control::Text("/bin/zsh".to_owned()));
    assert_eq!(press(&mut page, NamedKey::Escape), SettingsOutcome::Close);
}

#[test]
fn tab_in_the_editor_reverts_and_moves_to_the_sections() {
    let mut page = page("");
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    type_text(&mut page, "x");
    assert_eq!(press(&mut page, NamedKey::Tab), SettingsOutcome::Revert);
    assert!(page.sheet().sections_focused);
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn an_invalid_value_shows_an_inline_error_and_keeps_the_editor() {
    let mut page = page("");
    focus_key(&mut page, "font.size");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    type_text(&mut page, "big");
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Pending);
    assert_eq!(
        error(&page).as_deref(),
        Some("enter a number greater than 0")
    );
    assert!(matches!(control(&page), Control::Editor(_)));
    press(&mut page, NamedKey::Backspace);
    assert_eq!(error(&page), None);
}

#[test]
fn an_unchanged_value_reverts_without_saving() {
    let mut page = page("");
    focus_key(&mut page, "font.size");
    press(&mut page, NamedKey::Enter);
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Revert);
    assert_eq!(control(&page), Control::Number("13".to_owned()));
}

#[test]
fn an_unset_colour_opens_an_empty_editor() {
    let mut page = page("");
    page.focus(1);
    press(&mut page, NamedKey::Enter);
    assert_eq!(
        control(&page),
        Control::Editor(Field {
            text: String::new(),
            cursor: 0,
            placeholder: "unset",
        })
    );
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Revert);
    press(&mut page, NamedKey::Enter);
    let red = Value::Text("#ff0000".to_owned());
    assert_eq!(
        type_text(&mut page, "#ff0000"),
        preview_of("", "colors.background", Some(&red))
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("colors.background", Some(red))
    );
}

#[test]
fn clearing_a_colour_from_the_file_removes_it() {
    let text = "[colors]\nbackground = \"#102030\"\n";
    let mut page = page(text);
    page.focus(1);
    press(&mut page, NamedKey::Enter);
    assert_eq!(
        control(&page),
        Control::Editor(Field {
            text: "#102030".to_owned(),
            cursor: 7,
            placeholder: "unset",
        })
    );
    assert_eq!(
        page.key(&ctrl('u')),
        preview_of(text, "colors.background", None)
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("colors.background", None)
    );
}

#[test]
fn delete_resets_only_entries_from_the_file() {
    let mut page = page("[font]\nsize = 14\n");
    focus_key(&mut page, "font.size");
    assert_eq!(press(&mut page, NamedKey::Delete), save("font.size", None));
    page.focus(2);
    assert_eq!(press(&mut page, NamedKey::Delete), SettingsOutcome::Pending);
}

#[test]
fn delete_drops_a_pending_value() {
    let mut page = page("[font]\nsize = 14\n");
    focus_key(&mut page, "font.size");
    press(&mut page, NamedKey::Right);
    assert_eq!(press(&mut page, NamedKey::Delete), save("font.size", None));
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn backspace_never_resets() {
    let mut page = page("[font]\nsize = 14\n");
    focus_key(&mut page, "font.size");
    assert_eq!(
        press(&mut page, NamedKey::Backspace),
        SettingsOutcome::Pending
    );
    assert_eq!(control(&page), Control::Number("14".to_owned()));
}

#[test]
fn typing_searches_all_sections_with_headings() {
    let mut page = page("");
    press(&mut page, NamedKey::Down);
    assert_eq!(type_text(&mut page, "pad"), SettingsOutcome::Pending);
    let sheet = page.sheet();
    assert_eq!(sheet.search.text, "pad");
    assert_eq!(sheet.search.cursor, 3);
    assert_eq!(sheet.crumb, "Search");
    assert_eq!(sheet.section, None);
    assert_eq!(sheet.focus, Some(0));
    assert_eq!(sheet.groups.len(), 1);
    assert_eq!(sheet.groups[0].heading, Some("Window"));
    assert_eq!(titles(&sheet), ["Padding X", "Padding Y"]);
    page.key(&typed(' '));
    assert_eq!(page.sheet().search.text, "pad ");
    press(&mut page, NamedKey::Backspace);
    press(&mut page, NamedKey::Backspace);
    assert_eq!(page.sheet().search.text, "pa");
    page.key(&ctrl('u'));
    let sheet = page.sheet();
    assert_eq!(sheet.search.text, "");
    assert_eq!(sheet.section, Some(0));
    assert_eq!(sheet.crumb, "Appearance");
}

#[test]
fn search_groups_settings_by_section_then_bindings() {
    let mut page = page("");
    type_text(&mut page, "size");
    let sheet = page.sheet();
    let headings: Vec<_> = sheet.groups.iter().map(|group| group.heading).collect();
    assert_eq!(headings, [Some("Font")]);
    assert_eq!(titles(&sheet), ["Size", "Line height", "UI size"]);
    page.key(&ctrl('u'));
    type_text(&mut page, "split");
    let sheet = page.sheet();
    assert_eq!(sheet.groups.len(), 1);
    assert_eq!(sheet.groups[0].heading, Some("Key bindings"));
    assert_eq!(sheet.groups[0].entries.len(), 4);
    page.key(&ctrl('u'));
    type_text(&mut page, "prefix");
    let headings: Vec<_> = page
        .sheet()
        .groups
        .iter()
        .map(|group| group.heading)
        .collect();
    assert_eq!(headings, [Some("Keys"), Some("Key bindings")]);
}

#[test]
fn slash_with_an_empty_query_is_ignored() {
    let mut page = page("");
    assert_eq!(page.key(&typed('/')), SettingsOutcome::Pending);
    assert_eq!(page.sheet().search.text, "");
    type_text(&mut page, "a/");
    assert_eq!(page.sheet().search.text, "a/");
}

#[test]
fn tab_is_ignored_while_searching() {
    let mut page = page("");
    type_text(&mut page, "pad");
    assert_eq!(press(&mut page, NamedKey::Tab), SettingsOutcome::Pending);
    assert!(!page.sheet().sections_focused);
}

#[test]
fn typing_in_the_sections_column_searches_the_entries() {
    let mut page = page("");
    press(&mut page, NamedKey::Tab);
    type_text(&mut page, "pad");
    let sheet = page.sheet();
    assert!(!sheet.sections_focused);
    assert_eq!(sheet.search.text, "pad");
    assert_eq!(sheet.focus, Some(0));
}

#[test]
fn binding_rows_ignore_edit_keys() {
    let mut page = page("");
    type_text(&mut page, "split");
    for name in [
        NamedKey::Enter,
        NamedKey::Delete,
        NamedKey::Left,
        NamedKey::Right,
    ] {
        assert_eq!(press(&mut page, name), SettingsOutcome::Pending);
    }
    assert_eq!(page.sheet().keys, default_keys());
}

#[test]
fn insert_goes_to_the_query_or_the_editor() {
    let mut page = page("");
    assert_eq!(page.insert("pa\nd"), SettingsOutcome::Pending);
    assert_eq!(page.sheet().search.text, "pad");
    page.key(&ctrl('u'));
    focus_key(&mut page, "window.padding_y");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    assert_eq!(
        page.insert("5"),
        preview_of("", "window.padding_y", Some(&Value::Integer(5)))
    );
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("window.padding_y", Some(Value::Integer(5)))
    );
}

#[path = "settings_page_events_tests.rs"]
mod events;
