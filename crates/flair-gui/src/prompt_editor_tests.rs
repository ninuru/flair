use super::*;
use crate::{
    CellMetrics,
    font::UiMetrics,
    overlay_layout::{OverlayInput, OverlayScene},
    overlays::Overlays,
    renderer::PxRect,
    settings_layout::SettingsHit,
    settings_page::{SettingsOutcome, SettingsPage},
};
use flair_core::{
    config::Config,
    config_edit::apply,
    key::{Key, KeyAction, KeyInput, Locks, Mods},
    proto::ServerMsg,
    status::StatusLine,
};
use std::path::Path;

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
    let mut config = Config::default();
    config.prompt.path = flair_core::config::PromptPath::Full;
    let mut page = SettingsPage::new(&config, config.theme(), String::new(), Vec::new());
    page.click(SettingsHit::Section(4));
    page.click(SettingsHit::PromptEditor(PromptAction::Path));
    page.click(SettingsHit::PromptEditor(PromptAction::Colour));
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
    for _ in 0..128 {
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
                    | SettingsHit::Choose(_)
                    | SettingsHit::DismissDropdown
                    | SettingsHit::Cycle(_)
                    | SettingsHit::Edit(_) => None,
                }
            })
            .collect();
        assert!(visible.contains(&PromptAction::Save));
        actions.extend(visible);
        page.set_prompt_scroll_limit(scene.hits.prompt_scroll_limit.unwrap());
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

#[test]
fn editor_disables_unchanged_save_and_moves_past_the_first_segment() {
    let mut config = Config::default();
    config.prompt.order = PromptOrder::from_chips(vec![Chip::Cwd, Chip::Docker]).unwrap();
    let mut editor = PromptEditor::new(&config.prompt);
    assert!(!editor.available(PromptAction::Save));
    assert!(!editor.available(PromptAction::Cancel));
    assert!(!editor.available(PromptAction::Move(-1)));
    assert!(editor.available(PromptAction::Move(1)));
    editor.act(PromptAction::Path);
    assert!(editor.available(PromptAction::Save));
    assert!(editor.available(PromptAction::Cancel));
}

fn polished_scene(editor: &PromptEditor, hover: Option<(f32, f32)>) -> OverlayScene {
    let config = Config::default();
    let mut page = SettingsPage::new(&config, config.theme(), String::new(), Vec::new());
    page.click(SettingsHit::Section(4));
    let theme = config.theme();
    let metrics = CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    };
    let input = OverlayInput {
        px: (1280, 900),
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
        hover,
    };
    editor.layout(&page.sheet(), &input, metrics, &mut |text, _| {
        text.chars().count() as f32 * 7.0
    })
}

fn action_rect(scene: &OverlayScene, action: PromptAction) -> Option<PxRect> {
    scene.hits.controls.iter().find_map(|(hit, rect)| {
        matches!(hit, SettingsHit::PromptEditor(found) if *found == action).then_some(*rect)
    })
}

#[test]
fn save_is_only_interactive_when_dirty_and_hover_keeps_its_position() {
    let mut editor = PromptEditor::new(&PromptConfig::default());
    let saved = polished_scene(&editor, None);
    assert!(action_rect(&saved, PromptAction::Save).is_none());
    assert!(saved.text.iter().any(|run| run.text == "Saved"));
    editor.act(PromptAction::Path);
    let normal = polished_scene(&editor, None);
    let rect = action_rect(&normal, PromptAction::Save).unwrap();
    let hover = polished_scene(&editor, Some((rect.x + 1.0, rect.y + 1.0)));
    assert_eq!(action_rect(&hover, PromptAction::Save), Some(rect));
    let fill = |scene: &OverlayScene| {
        scene
            .quads
            .iter()
            .find(|quad| quad.rect == rect)
            .unwrap()
            .color
    };
    assert_ne!(fill(&normal), fill(&hover));
    let cancel = action_rect(&normal, PromptAction::Cancel).unwrap();
    assert_ne!(
        fill(&normal),
        normal
            .quads
            .iter()
            .find(|quad| quad.rect == cancel)
            .unwrap()
            .color
    );
}

#[test]
fn an_empty_preview_explains_how_to_recover() {
    let mut editor = PromptEditor::new(&PromptConfig::default());
    for chip in Chip::ALL {
        if editor.draft.enabled(chip) {
            editor.act(PromptAction::Select(chip));
            editor.act(PromptAction::Toggle);
        }
    }
    let scene = polished_scene(&editor, None);
    assert!(
        scene
            .text
            .iter()
            .any(|run| run.text.starts_with("No segments selected"))
    );
    assert!(action_rect(&scene, PromptAction::Add(Chip::Cwd)).is_some());
    editor.act(PromptAction::Style);
    let scene = polished_scene(&editor, None);
    assert!(
        scene
            .text
            .iter()
            .any(|run| run.text.contains("shell’s prompt (PS1)"))
    );
}

#[test]
fn every_option_remains_reachable_in_narrow_and_short_windows() {
    for size in [(640, 600), (960, 500)] {
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
            px: size,
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
        let mut seen = Vec::new();
        for _ in 0..128 {
            let scene =
                page.prompt_editor()
                    .layout(&page.sheet(), &input, metrics, &mut |text, _| {
                        text.chars().count() as f32 * 7.0
                    });
            for action in [
                PromptAction::Colour,
                PromptAction::Appearance,
                PromptAction::Icon,
                PromptAction::Path,
                PromptAction::Time,
                PromptAction::Date,
                PromptAction::Separator,
                PromptAction::Style,
                PromptAction::Duration(-1),
                PromptAction::Duration(1),
            ] {
                if action_rect(&scene, action).is_some() {
                    seen.push(action);
                }
            }
            page.set_prompt_scroll_limit(scene.hits.prompt_scroll_limit.unwrap());
            page.wheel(1);
        }
        for action in [
            PromptAction::Colour,
            PromptAction::Appearance,
            PromptAction::Icon,
            PromptAction::Path,
            PromptAction::Time,
            PromptAction::Date,
            PromptAction::Separator,
            PromptAction::Style,
            PromptAction::Duration(-1),
            PromptAction::Duration(1),
        ] {
            assert!(seen.contains(&action), "{size:?}: {action:?}");
        }
    }
}

#[test]
fn rendered_scroll_bounds_reach_the_end_and_reverse_without_a_dead_zone() {
    let config = Config::default();
    let mut overlays = Overlays::default();
    overlays.open(
        &ServerMsg::Settings,
        &StatusLine::default(),
        &config,
        Path::new("/nonexistent/flair.toml"),
    );
    let page = overlays.settings_mut().unwrap();
    page.click(SettingsHit::Section(4));
    page.click(SettingsHit::PromptEditor(PromptAction::Path));
    let theme = config.theme();
    let metrics = CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    };
    let input = OverlayInput {
        px: (640, 600),
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
    let render = |overlays: &Overlays| {
        overlays
            .layout(&input, &mut |text, _| text.chars().count() as f32 * 7.0)
            .unwrap()
    };
    let initial = render(&overlays);
    overlays.place(Some(initial.hits));
    overlays.wheel(i32::MAX);
    let end = render(&overlays);
    let end_rect = action_rect(&end, PromptAction::Duration(1)).unwrap();
    overlays.place(Some(end.hits));
    overlays.wheel(-1);
    let back = render(&overlays);
    let back_rect = action_rect(&back, PromptAction::Duration(1)).unwrap();
    assert!(back_rect.y > end_rect.y);
}
