use super::*;
use flair_core::{config::Config, config_edit::apply};

#[test]
fn segment_edits_preview_without_changing_saved_configuration() {
    let config = Config::default();
    let mut editor = PromptEditor::new(&config.prompt);
    editor.act(PromptAction::Select(Chip::Docker));
    editor.act(PromptAction::Toggle);
    editor.act(PromptAction::Colour);
    editor.act(PromptAction::Move(-1));
    assert!(editor.draft.enabled(Chip::Docker));
    assert!(!config.prompt.docker);
    assert!(editor.draft.segments[&Chip::Docker].colour.is_some());
    let order = editor.ordered();
    assert!(
        order.iter().position(|chip| *chip == Chip::Docker).unwrap()
            < Chip::ALL
                .iter()
                .position(|chip| *chip == Chip::Docker)
                .unwrap()
    );
    let (text, saved) = editor.changes().iter().fold(
        (String::new(), Config::default()),
        |(text, _), (key, value)| apply(&text, key, value.as_ref()).unwrap(),
    );
    assert!(text.contains("[prompt.segments.docker]"));
    assert_eq!(saved.prompt, editor.draft);
}

#[test]
fn reset_removes_overrides_and_cancel_restores_saved_prompt() {
    let config =
        Config::from_toml("[prompt]\npath='folder'\n[prompt.segments.cwd]\ncolour='red'\n")
            .unwrap();
    let mut editor = PromptEditor::new(&config.prompt);
    editor.act(PromptAction::Reset);
    assert_eq!(editor.draft, PromptConfig::default());
    editor.cancel();
    assert_eq!(editor.draft, config.prompt);
}

#[test]
fn editor_actions_reach_the_settings_save_and_cancel_flow() {
    use crate::{
        settings_layout::SettingsHit,
        settings_page::{SettingsOutcome, SettingsPage},
    };
    let config = Config::default();
    let mut page = SettingsPage::new(&config, config.theme(), String::new(), Vec::new());
    page.click(SettingsHit::Section(4));
    assert!(page.is_prompt_editor());
    page.click(SettingsHit::PromptEditor(PromptAction::Select(Chip::Date)));
    let preview = page.click(SettingsHit::PromptEditor(PromptAction::Toggle));
    assert!(matches!(preview, SettingsOutcome::Preview(config) if config.prompt.date));
    let outcome = page.click(SettingsHit::PromptEditor(PromptAction::Save));
    let SettingsOutcome::SavePrompt(changes) = outcome else {
        panic!("expected atomic prompt save")
    };
    let mut text = String::new();
    for (key, value) in changes {
        text = apply(&text, &key, value.as_ref()).unwrap().0;
    }
    let saved = Config::from_toml(&text).unwrap();
    page.refresh(&saved, saved.theme(), text);
    assert!(!page.prompt_editor().dirty());
    page.click(SettingsHit::PromptEditor(PromptAction::Reset));
    assert!(page.prompt_editor().dirty());
    assert_eq!(
        page.click(SettingsHit::PromptEditor(PromptAction::Cancel)),
        SettingsOutcome::Revert
    );
    assert!(page.prompt_editor().draft.date);
}

#[test]
fn compact_editor_keeps_save_visible_and_scrolls_to_every_option() {
    use crate::{font::UiMetrics, settings_layout::SettingsHit, settings_page::SettingsPage};
    let config = Config::default();
    let mut page = SettingsPage::new(&config, config.theme(), String::new(), Vec::new());
    page.click(SettingsHit::Section(4));
    page.click(SettingsHit::PromptEditor(PromptAction::Path));
    let theme = config.theme();
    let metrics = CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    };
    let input = OverlayInput {
        px: (960, 600),
        scale: 1.0,
        metrics,
        ui: UiMetrics {
            height: 16.0,
            ascent: 12.0,
            small: 0.7,
            cap: 8.0,
            icon: metrics,
        },
        theme: &theme,
        hover: None,
    };
    let mut actions = Vec::new();
    for _ in 0..8 {
        let scene = page
            .prompt_editor()
            .layout(&page.sheet(), &input, metrics, &mut |text, _| {
                text.chars().count() as f32 * 7.0
            });
        assert!(scene.text.iter().any(|run| run.text == "~/projects/flair"));
        let visible: Vec<_> = scene
            .hits
            .controls
            .iter()
            .filter_map(|(hit, rect)| {
                assert!(rect.y >= 0.0 && rect.y + rect.h <= 600.0);
                match hit {
                    SettingsHit::PromptEditor(action) => Some(*action),
                    SettingsHit::Section(_)
                    | SettingsHit::Close
                    | SettingsHit::Search
                    | SettingsHit::Toggle(_)
                    | SettingsHit::Decrease(_)
                    | SettingsHit::Increase(_)
                    | SettingsHit::Cycle(_)
                    | SettingsHit::Edit(_) => None,
                }
            })
            .collect();
        assert!(visible.contains(&PromptAction::Save));
        actions.extend(visible);
        page.wheel(1);
    }
    for action in [
        PromptAction::Colour,
        PromptAction::Appearance,
        PromptAction::Icon,
        PromptAction::Move(-1),
        PromptAction::Move(1),
        PromptAction::Path,
        PromptAction::Time,
        PromptAction::Date,
        PromptAction::Separator,
        PromptAction::GlobalAppearance,
        PromptAction::GlobalIcons,
        PromptAction::Style,
        PromptAction::Duration(-1),
        PromptAction::Reset,
        PromptAction::Cancel,
    ] {
        assert!(actions.contains(&action), "{action:?}");
    }
}

#[test]
fn adding_a_segment_from_the_tray_enables_and_selects_it() {
    let mut editor = PromptEditor::new(&PromptConfig::default());
    editor.act(PromptAction::Add(Chip::Date));
    assert_eq!(editor.selected(), Chip::Date);
    assert!(editor.draft.date);
}

#[test]
fn search_typing_from_the_prompt_editor_never_runs_segment_shortcuts() {
    use crate::{
        settings_layout::SettingsHit,
        settings_page::{SettingsOutcome, SettingsPage},
    };
    use flair_core::key::{Key, KeyAction, KeyInput, Locks, Mods};
    let config = Config::default();
    let mut page = SettingsPage::new(&config, config.theme(), String::new(), Vec::new());
    page.click(SettingsHit::Section(4));
    page.click(SettingsHit::PromptEditor(PromptAction::Colour));
    let draft = page.prompt_editor().draft.clone();
    page.click(SettingsHit::Search);
    for c in "color".chars() {
        let outcome = page.key(&KeyInput {
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
        });
        assert!(!matches!(outcome, SettingsOutcome::Preview(_)));
    }
    assert_eq!(page.sheet().search.text, "color");
    assert_eq!(page.prompt_editor().draft, draft);
    assert!(!page.is_prompt_editor());
    assert!(!page.sheet().groups.is_empty());
}
