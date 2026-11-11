use super::*;
use flair_core::key::{Locks, Mods};

fn menu(chip: Chip) -> PromptMenu {
    PromptMenu::new(
        PaneId(3),
        chip,
        &PromptInfo {
            cwd: Some("/srv/it's safe".into()),
            docker: Some("prod".into()),
            ..PromptInfo::default()
        },
        PxRect {
            x: 100.0,
            y: 100.0,
            w: 60.0,
            h: 24.0,
        },
        "value".into(),
    )
}

fn key(key: Key) -> KeyInput {
    KeyInput {
        key,
        action: KeyAction::Press,
        mods: Mods::default(),
        locks: Locks::default(),
        text: None,
        repeat: false,
        keypad: false,
        unshifted: None,
        shifted: None,
        base: None,
    }
}

#[test]
fn docker_menu_search_selects_a_context_without_shell_interpolation() {
    let mut menu = menu(Chip::Docker);
    menu.contexts(Ok(vec!["default".into(), "dev".into(), "prod".into()]));
    menu.insert("DE");
    assert_eq!(
        menu.filtered()
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "dev"]
    );
    menu.key(&key(Key::Named(NamedKey::Down)));
    assert_eq!(
        menu.key(&key(Key::Named(NamedKey::Enter))),
        MenuOutcome::Action(PromptAction::DockerContext("dev".into()))
    );
}

#[test]
fn directory_actions_copy_the_full_path_and_quote_inserted_commands() {
    let menu = menu(Chip::Cwd);
    assert_eq!(
        menu.choose(0),
        MenuOutcome::Action(PromptAction::Copy("/srv/it's safe".into()))
    );
    assert_eq!(
        menu.choose(1),
        MenuOutcome::Action(PromptAction::Insert {
            pane: PaneId(3),
            text: "cd -- '/srv/it'\\''s safe'".into()
        })
    );
}

#[test]
fn failed_context_discovery_keeps_a_copy_action_and_shows_the_error() {
    let mut menu = menu(Chip::Docker);
    menu.contexts(Err("Docker is unavailable".into()));
    assert_eq!(menu.error.as_deref(), Some("Docker is unavailable"));
    assert_eq!(
        menu.choose(0),
        MenuOutcome::Action(PromptAction::Copy("prod".into()))
    );
}

#[test]
fn closing_or_editing_the_query_never_runs_an_action() {
    let mut menu = menu(Chip::Docker);
    assert_eq!(
        menu.key(&key(Key::Named(NamedKey::Escape))),
        MenuOutcome::Close
    );
    menu.contexts(Ok(vec!["dev".into()]));
    menu.insert("no match");
    assert_eq!(
        menu.key(&key(Key::Named(NamedKey::Enter))),
        MenuOutcome::Pending
    );
    assert_eq!(menu.choose(999), MenuOutcome::Pending);
}

#[test]
fn context_names_are_bounded_and_cannot_contain_terminal_controls() {
    assert_eq!(
        context_names("default\nprod\nprod\n\u{1b}bad\n-evil\n"),
        ["default", "prod"]
    );
}

fn mirror_with_input(text: &str, alt_screen: bool) -> crate::mirror::Mirror {
    use flair_core::{
        geom::{CellPoint, GridSize},
        proto::{Cell, Cursor, CursorShape, PaneFlags, PaneUpdate},
    };
    let mut mirror = crate::mirror::Mirror::default();
    let mut row = vec![Cell::default(); 20];
    for (cell, c) in row.iter_mut().zip(text.chars()) {
        cell.c = c;
    }
    mirror.apply(&[PaneUpdate {
        pane: PaneId(3),
        size: GridSize { cols: 20, rows: 3 },
        full: true,
        rows: vec![(2, row)],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 2 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags {
            prompt_row: Some(1),
            alt_screen,
            ..PaneFlags::default()
        },
        selection: vec![],
    }]);
    mirror
}

#[test]
fn command_insertion_requires_the_originating_active_empty_shell_prompt() {
    assert!(can_insert(
        PaneId(3),
        Some(PaneId(3)),
        &mirror_with_input("", false)
    ));
    assert!(!can_insert(
        PaneId(3),
        Some(PaneId(4)),
        &mirror_with_input("", false)
    ));
    assert!(!can_insert(
        PaneId(3),
        Some(PaneId(3)),
        &mirror_with_input("existing command", false)
    ));
    assert!(!can_insert(
        PaneId(3),
        Some(PaneId(3)),
        &mirror_with_input("", true)
    ));
    assert!(!can_insert(PaneId(3), None, &mirror_with_input("", false)));
}

fn input(
    px: (u32, u32),
    theme: &flair_core::theme::Theme,
) -> crate::overlay_layout::OverlayInput<'_> {
    crate::overlay_layout::OverlayInput {
        metrics: crate::CellMetrics {
            width: 10.0,
            height: 24.0,
            baseline: 18.0,
        },
        px,
        scale: 1.0,
        ui: crate::UiMetrics {
            height: 16.0,
            ascent: 12.0,
            small: 0.8,
            cap: 10.0,
            icon: crate::CellMetrics {
                width: 8.0,
                height: 16.0,
                baseline: 12.0,
            },
        },
        theme,
        hover: None,
    }
}

#[test]
fn filtered_mouse_rows_choose_the_visible_context() {
    let mut menu = menu(Chip::Docker);
    menu.contexts(Ok(vec!["default".into(), "dev".into(), "prod".into()]));
    menu.insert("prod");
    let theme = flair_core::config::Config::default().theme();
    let scene = menu.layout(&input((800, 600), &theme), &mut |text, _| {
        text.len() as f32 * 8.0
    });
    let (_, row) = scene.hits.rows[0];
    let hit = scene
        .hits
        .hit_test((f64::from(row.x + 2.0), f64::from(row.y + 2.0)));
    assert_eq!(hit, crate::overlay_layout::OverlayHit::Row(0));
    assert_eq!(
        menu.choose(0),
        MenuOutcome::Action(PromptAction::DockerContext("prod".into()))
    );
}

#[test]
fn popover_stays_inside_short_narrow_windows_and_scrolls_selection_into_view() {
    let mut menu = menu(Chip::Docker);
    menu.contexts(Ok((0..40)
        .map(|index| format!("context-{index}"))
        .collect()));
    menu.selected = 39;
    let theme = flair_core::config::Config::default().theme();
    let scene = menu.layout(&input((240, 160), &theme), &mut |text, _| {
        text.len() as f32 * 8.0
    });
    let panel = scene.hits.panel;
    assert!(
        panel.x >= 0.0
            && panel.y >= 0.0
            && panel.x + panel.w <= 240.0
            && panel.y + panel.h <= 160.0
    );
    assert!(scene.hits.rows.iter().any(|(index, _)| *index == 39));
    for (_, row) in scene.hits.rows {
        assert!(row.y >= panel.y && row.y + row.h <= panel.y + panel.h);
    }
}

#[test]
fn later_blank_process_output_cannot_receive_inserted_commands() {
    use flair_core::{
        geom::{CellPoint, GridSize},
        proto::{Cursor, CursorShape, PaneFlags, PaneUpdate},
    };
    let mut mirror = mirror_with_input("", false);
    mirror.apply(&[PaneUpdate {
        pane: PaneId(3),
        size: GridSize { cols: 20, rows: 3 },
        full: false,
        rows: vec![],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 2 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags {
            prompt_row: Some(0),
            ..PaneFlags::default()
        },
        selection: vec![],
    }]);
    assert!(!can_insert(PaneId(3), Some(PaneId(3)), &mirror));
}

#[test]
fn docker_context_rows_use_brand_icons_and_a_neutral_inverted_selection() {
    let mut menu = menu(Chip::Docker);
    menu.contexts(Ok(vec!["prod".into()]));
    let theme = flair_core::config::Config::default().theme();
    let scene = menu.layout(&input((800, 600), &theme), &mut |text, _| {
        text.len() as f32 * 8.0
    });
    let row = scene.hits.rows[0].1;
    assert!(
        scene
            .quads
            .iter()
            .any(|quad| quad.rect == row
                && quad.color == crate::chrome_bar::opaque(theme.foreground))
    );
    assert!(scene.text.iter().any(|run| run.text == "\u{f308}"
        && run.face == crate::renderer::TextFace::PromptIcon
        && run.color == theme.background));
    assert!(
        scene
            .text
            .iter()
            .any(|run| run.text == "prod" && run.color == theme.background)
    );
}
