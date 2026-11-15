use super::*;

fn with_icons(mut status: StatusParts, icon: char) -> StatusParts {
    for tab in &mut status.tabs {
        tab.icon = Some(icon);
    }
    status
}

fn icons(layout: &ChromeLayout) -> Vec<&TextRun> {
    layout
        .chrome
        .text
        .iter()
        .filter(|run| run.face == TextFace::PromptIcon && run.text != FILES_KEY.to_string())
        .collect()
}

#[test]
fn a_tab_icon_follows_the_name_and_widens_the_tab() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut status = parts();
    status.tabs[0].icon = Some('\u{f120}');
    status.tabs[1].icon = Some('\u{f308}');
    let layout = Case::new(&status).run();
    let found: Vec<_> = icons(&layout)
        .iter()
        .map(|run| (run.x, run.baseline, run.text.as_str(), run.color, run.bold))
        .collect();
    assert_eq!(
        found,
        [
            (126.0, 19.0, "\u{f120}", surfaces.bright, false),
            (219.0, 19.0, "\u{f308}", surfaces.muted, false),
        ]
    );
    let order: Vec<&str> = layout.chrome.text[2..]
        .iter()
        .map(|run| run.text.as_str())
        .filter(|text| *text != FILES_KEY.to_string())
        .collect();
    assert_eq!(order, ["1", "shell", "\u{f120}", "2", "editor", "\u{f308}"]);
    let shell = text(&layout, "shell").unwrap();
    assert_eq!(
        (shell.x, shell.baseline, shell.color),
        (85.0, 21.0, theme.foreground)
    );
    let editor = text(&layout, "editor").unwrap();
    assert_eq!((editor.x, editor.color), (171.0, surfaces.muted));
    assert_eq!(layout.hits[1].rect, rect(63.0, 0.0, 82.0, 34.0));
    assert_eq!(layout.hits[2].rect, rect(149.0, 0.0, 89.0, 34.0));
    assert!(quad_at(&layout, rect(63.0, 5.0, 82.0, 24.0)).is_some());
    let hovered = Case {
        hover: Some((200.0, 10.0)),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(icons(&hovered)[1].color, surfaces.muted);
    assert_eq!(text(&hovered, "editor").unwrap().color, theme.foreground);
}

#[test]
fn tabs_without_an_icon_keep_their_width_and_draw_no_icon() {
    let mut status = parts();
    status.tabs[1].icon = Some('\u{f308}');
    let layout = Case::new(&status).run();
    assert_eq!(icons(&layout).len(), 1);
    assert_eq!(layout.hits[1].rect, rect(63.0, 0.0, 65.0, 34.0));
    assert_eq!(text(&layout, "shell").unwrap().x, 85.0);
    assert_eq!(layout.hits[2].rect, rect(132.0, 0.0, 89.0, 34.0));
}

#[test]
fn overflowing_and_ellipsized_tabs_keep_their_icons_inside_the_tab() {
    for (width, active) in [
        (291, 30),
        (191, 30),
        (151, 30),
        (291, 1),
        (291, 15),
        (991, 2),
    ] {
        let status = with_icons(many(30, active), '\u{f120}');
        let width = width + 30;
        let layout = Case {
            width,
            ..Case::new(&status)
        }
        .run();
        let tabs: Vec<&Hit> = layout.hits[1..]
            .iter()
            .filter(|hit| matches!(hit.target, HitTarget::Window(_)))
            .collect();
        let icons = icons(&layout);
        assert_eq!(icons.len(), tabs.len(), "{width} {active}");
        assert!(!tabs.is_empty(), "{width} {active}");
        for (hit, icon) in tabs.iter().zip(&icons) {
            let at = layout
                .chrome
                .text
                .iter()
                .position(|run| std::ptr::eq(run, *icon))
                .unwrap();
            let name = &layout.chrome.text[at - 1];
            let index = &layout.chrome.text[at - 2];
            assert_eq!(
                name.x,
                index.x + measure(&format!("{} ", index.text), false)
            );
            assert_eq!(
                icon.x,
                name.x + measure(&name.text, false) + 6.0,
                "{width} {active}"
            );
            assert!(
                icon.x + 11.0 <= hit.rect.x + hit.rect.w - 8.0,
                "{width} {active} {}",
                name.text
            );
        }
        let last = tabs.last().unwrap().rect;
        assert!(last.x + last.w <= width as f32 - 8.0, "{width} {active}");
    }
}

#[test]
fn tab_icons_can_lead_the_name_or_be_switched_off() {
    let mut status = parts();
    status.tabs[0].icon = Some('\u{f120}');
    let before_status = StatusParts {
        tab_icon: Some(IconSide::Before),
        ..status.clone()
    };
    let before = Case::new(&before_status).run();
    assert_eq!(icons(&before)[0].x, 85.0);
    assert_eq!(text(&before, "shell").unwrap().x, 102.0);
    assert_eq!(before.hits[1].rect, rect(63.0, 0.0, 82.0, 34.0));
    let off_status = StatusParts {
        tab_icon: None,
        ..status
    };
    let off = Case::new(&off_status).run();
    assert!(icons(&off).is_empty());
    assert_eq!(text(&off, "shell").unwrap().x, 85.0);
    assert_eq!(off.hits[1].rect, rect(63.0, 0.0, 65.0, 34.0));
}
