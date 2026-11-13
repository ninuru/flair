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
        .filter(|run| run.face == TextFace::PromptIcon)
        .collect()
}

#[test]
fn a_tab_icon_sits_between_the_index_and_the_name_and_widens_the_tab() {
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
            (97.0, 19.0, "\u{f120}", surfaces.bright, false),
            (189.0, 19.0, "\u{f308}", surfaces.muted, false),
        ]
    );
    let order: Vec<&str> = layout.chrome.text[2..]
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(order, ["1", "\u{f120}", "shell", "2", "\u{f308}", "editor"]);
    let shell = text(&layout, "shell").unwrap();
    assert_eq!(
        (shell.x, shell.baseline, shell.color),
        (114.0, 21.0, theme.foreground)
    );
    let editor = text(&layout, "editor").unwrap();
    assert_eq!((editor.x, editor.color), (206.0, surfaces.muted));
    assert_eq!(layout.hits[1].rect, rect(73.0, 0.0, 86.0, 34.0));
    assert_eq!(layout.hits[2].rect, rect(165.0, 0.0, 93.0, 34.0));
    assert!(quad_at(&layout, rect(81.0, 32.0, 70.0, 2.0)).is_some());
    let hovered = Case {
        hover: Some((200.0, 10.0)),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(icons(&hovered)[1].color, surfaces.muted);
    assert_eq!(text(&hovered, "editor").unwrap().color, surfaces.bright);
}

#[test]
fn tabs_without_an_icon_keep_their_width_and_draw_no_icon() {
    let mut status = parts();
    status.tabs[1].icon = Some('\u{f308}');
    let layout = Case::new(&status).run();
    assert_eq!(icons(&layout).len(), 1);
    assert_eq!(layout.hits[1].rect, rect(73.0, 0.0, 69.0, 34.0));
    assert_eq!(text(&layout, "shell").unwrap().x, 97.0);
    assert_eq!(layout.hits[2].rect, rect(148.0, 0.0, 93.0, 34.0));
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
            let index = &layout.chrome.text[at - 1];
            let name = &layout.chrome.text[at + 1];
            assert_eq!(
                icon.x,
                index.x + measure(&format!("{} ", index.text), false)
            );
            assert_eq!(name.x, icon.x + 17.0, "{width} {active}");
            assert!(
                name.x + measure(&name.text, false) <= hit.rect.x + hit.rect.w - 10.0,
                "{width} {active} {}",
                name.text
            );
        }
        let last = tabs.last().unwrap().rect;
        assert!(last.x + last.w <= width as f32 - 8.0, "{width} {active}");
    }
}
