use super::*;
use flair_core::config::PromptStyle;

fn reload(page: &mut SettingsPage, text: &str) -> SettingsOutcome {
    let config = Config::from_toml(text).unwrap();
    page.refresh(&config, config.theme(), text.to_owned())
}

#[test]
fn a_stepper_click_on_another_entry_comes_back_as_a_preview() {
    let mut page = page("");
    section(&mut page, 2);
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.click(SettingsHit::Increase(1)),
        save("window.padding_x", Some(Value::Integer(9)))
    );
    let text = "[window]\npadding_x = 9\n";
    assert_eq!(
        reload(&mut page, text),
        preview_of(text, "window.padding_y", Some(&Value::Integer(1)))
    );
    assert_eq!(control(&page), Control::Number("1".to_owned()));
    assert_eq!(page.sheet().keys, pending_keys());
}

#[test]
fn an_edit_click_on_another_entry_opens_the_editor_after_the_save() {
    let mut page = page("");
    section(&mut page, 3);
    page.focus(2);
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.click(SettingsHit::Edit(0)),
        save("scrollback.lines", Some(Value::Integer(11000)))
    );
    assert_eq!(
        reload(&mut page, "[scrollback]\nlines = 11000\n"),
        SettingsOutcome::Pending
    );
    assert!(matches!(control(&page), Control::Editor(_)));
}

#[test]
fn a_failed_save_drops_the_queued_action() {
    let mut page = page("");
    section(&mut page, 5);
    page.focus(2);
    press(&mut page, NamedKey::Right);
    page.click(SettingsHit::Toggle(0));
    page.fail("keys.timeout_ms", "disk full");
    assert_eq!(reload(&mut page, ""), SettingsOutcome::Pending);
    assert_eq!(control(&page), Control::Toggle(true));
}

#[test]
fn delete_on_a_default_entry_drops_its_pending_value() {
    let mut page = page("");
    focus_key(&mut page, "font.size");
    press(&mut page, NamedKey::Right);
    assert_eq!(press(&mut page, NamedKey::Delete), SettingsOutcome::Revert);
    assert_eq!(control(&page), Control::Number("13".to_owned()));
    assert_eq!(page.sheet().keys, step_keys());
    assert_eq!(press(&mut page, NamedKey::Delete), SettingsOutcome::Pending);
}

#[test]
fn quoted_shell_arguments_reopen_unchanged() {
    let text = "[shell]\nargs = [\"-c\", \"echo hi\"]\n";
    let mut page = page(text);
    focus_key(&mut page, "shell.args");
    assert_eq!(control(&page), Control::Text("-c \"echo hi\"".to_owned()));
    press(&mut page, NamedKey::Enter);
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Revert);
    press(&mut page, NamedKey::Enter);
    type_text(&mut page, " -l");
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save(
            "shell.args",
            Some(Value::Words(
                ["-c", "echo hi", "-l"].map(str::to_owned).to_vec()
            ))
        )
    );
}

#[test]
fn an_unclosed_quote_keeps_the_editor_open() {
    let mut page = page("");
    focus_key(&mut page, "shell.args");
    press(&mut page, NamedKey::Enter);
    type_text(&mut page, "\"x");
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Pending);
    assert_eq!(error(&page).as_deref(), Some("close the quote"));
    assert!(matches!(control(&page), Control::Editor(_)));
}

#[test]
fn a_value_the_config_rejects_keeps_the_editor_open() {
    let mut page = page("");
    focus_key(&mut page, "keys.key");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    type_text(&mut page, "C-");
    let expected = apply("", "keys.key", Some(&Value::Text("C-".to_owned())))
        .unwrap_err()
        .to_string();
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Pending);
    assert_eq!(error(&page), Some(expected));
    assert!(matches!(control(&page), Control::Editor(_)));
}

#[test]
fn a_broken_file_shows_the_reason_inline() {
    let mut page = page("");
    page.refresh(
        &Config::default(),
        Config::default().theme(),
        "[font\n".to_owned(),
    );
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    type_text(&mut page, "x");
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Pending);
    let message = error(&page).unwrap();
    assert!(
        message.starts_with("flair.toml does not parse: "),
        "{message}"
    );
    assert!(!message.contains("TOML parse error"), "{message}");
    assert!(matches!(control(&page), Control::Editor(_)));
}

#[test]
fn clearing_a_text_entry_resets_it() {
    let text = "[shell]\nprogram = \"/bin/fish\"\n";
    let mut page = page(text);
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    assert_eq!(
        press(&mut page, NamedKey::Enter),
        save("shell.program", None)
    );
    let mut page = super::page("");
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    assert_eq!(press(&mut page, NamedKey::Enter), SettingsOutcome::Revert);
}

#[test]
fn the_editor_and_the_query_are_bounded() {
    let mut page = page("");
    page.insert(&"q".repeat(300));
    assert_eq!(page.sheet().search.text.len(), 256);
    page.key(&ctrl('u'));
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    page.insert(&"x".repeat(2000));
    match control(&page) {
        Control::Editor(field) => assert_eq!(field.text.len(), 1024),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_footer_follows_the_focused_entry() {
    let mut page = page("[font]\nsize = 14\n");
    assert_eq!(page.sheet().keys, step_keys());
    page.focus(1);
    assert_eq!(page.sheet().keys, edit_keys());
    focus_key(&mut page, "font.size");
    assert_eq!(page.sheet().keys, step_reset_keys());
    focus_key(&mut page, "keys.enabled");
    assert_eq!(page.sheet().keys, toggle_keys());
    page.focus(3);
    assert_eq!(page.sheet().keys, browse_keys());
    press(&mut page, NamedKey::Tab);
    assert_eq!(page.sheet().keys, sections_keys());
}

#[test]
fn the_sheet_marks_pending_entries_and_disabled_bindings() {
    let mut page = page("[keys]\nenabled = false\n");
    focus_key(&mut page, "keys.timeout_ms");
    press(&mut page, NamedKey::Right);
    let sheet = page.sheet();
    let pending: Vec<bool> = entries(&sheet)
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Setting { pending, .. } => Some(*pending),
            Entry::Binding { .. } => None,
        })
        .collect();
    assert_eq!(pending, [false, false, true]);
    assert!(sheet.groups.iter().any(|group| group.disabled));
    assert!(entries(&sheet).into_iter().all(|entry| match entry {
        Entry::Setting { .. } => true,
        Entry::Binding { disabled, .. } => *disabled,
    }));
    let page = super::page("");
    let sheet = page.sheet();
    assert!(sheet.groups.iter().all(|group| !group.disabled));
    let colours = entries(&sheet)
        .into_iter()
        .filter(|entry| {
            matches!(
                entry,
                Entry::Setting {
                    control: Control::Color(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(colours, 5);
}

#[test]
fn a_saved_prompt_style_writes_the_prompt_table() {
    let mut page = page("theme = \"gruvbox-dark\"\n");
    focus_key(&mut page, "prompt.style");
    press(&mut page, NamedKey::Right);
    let SettingsOutcome::Save { key, value } = press(&mut page, NamedKey::Enter) else {
        panic!("not saved");
    };
    assert_eq!(key, "prompt.style");
    let (text, config) = apply("theme = \"gruvbox-dark\"\n", key, value.as_ref()).unwrap();
    assert_eq!(
        text,
        "theme = \"gruvbox-dark\"\n\n[prompt]\nstyle = \"ps1\"\n"
    );
    assert_eq!(config.prompt.style, PromptStyle::Ps1);
    assert_eq!(config.theme, "gruvbox-dark");
    assert_eq!(reload(&mut page, &text), SettingsOutcome::Pending);
    assert_eq!(control(&page), Control::Choice("ps1".to_owned()));
    assert_eq!(source(&page), "flair.toml");
    assert_eq!(page.sheet().keys, step_reset_keys());
}

#[test]
fn the_theme_choice_still_previews_every_step() {
    let mut page = page("");
    let gruvbox = Value::Text("gruvbox-dark".to_owned());
    let mocha = Value::Text("catppuccin-mocha".to_owned());
    assert_eq!(
        press(&mut page, NamedKey::Right),
        preview_of("", "theme", Some(&gruvbox))
    );
    assert_eq!(
        reload(&mut page, ""),
        preview_of("", "theme", Some(&gruvbox))
    );
    assert_eq!(press(&mut page, NamedKey::Left), SettingsOutcome::Revert);
    assert_eq!(
        press(&mut page, NamedKey::Left),
        preview_of("", "theme", Some(&mocha))
    );
    assert_eq!(page.click(SettingsHit::Cycle(0)), SettingsOutcome::Pending);
    assert_eq!(page.sheet().dropdown.unwrap().selected, 0);
    assert_eq!(page.click(SettingsHit::Choose(1)), SettingsOutcome::Revert);
}
