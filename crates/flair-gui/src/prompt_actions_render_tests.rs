use super::*;
use crate::{
    Fonts,
    chrome_layout::{self, ChromeInput, HitTarget},
    compose::compose_frame,
    isles::Isles,
    mirror::Mirror,
    overlays::Overlays,
    renderer::{HeadlessContext, Renderer},
    status::status_parts,
};
use flair_core::{
    config::Config,
    geom::{CellPoint, CellRect},
    ids::WindowId,
    proto::{
        Cursor, CursorShape, PaneFlags, PanePlacement, PaneUpdate, PromptSnapshot, WindowLayout,
    },
    status::StatusLine,
};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

#[test]
fn gpu_prompt_menu_is_anchored_bounded_and_does_not_dim_the_terminal() {
    for (width, height, scale) in [(800, 480, 1.0), (360, 240, 1.0), (1200, 720, 1.5)] {
        let (ctx, gl) = match HeadlessContext::new((width, height)) {
            Ok(pair) => pair,
            Err(error) => {
                if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                    panic!("required EGL unavailable: {error}");
                }
                eprintln!("skipping prompt menu GPU test: {error}");
                return;
            }
        };
        let mut config = Config::default();
        config.font.family = "JetBrainsMono Nerd Font Mono".into();
        config.prompt.git_branch = false;
        config.prompt.docker = true;
        let theme = config.theme();
        let mut fonts = Fonts::load(&config.font, f64::from(scale)).unwrap();
        let metrics = fonts.metrics();
        let padding = (8.0 * scale, 4.0 * scale);
        let size = chrome_layout::grid_size((width, height), padding, scale, metrics);
        let row = if width == 360 {
            size.rows.saturating_sub(4)
        } else {
            1
        };
        let info = PromptInfo {
            cwd: Some("/home/developer".into()),
            docker: Some("skynetx-prod".into()),
            ..Default::default()
        };
        let mut mirror = Mirror::default();
        mirror.apply(&[PaneUpdate {
            pane: PaneId(1),
            size,
            full: true,
            rows: Vec::new(),
            cursor: Cursor {
                point: CellPoint {
                    col: 0,
                    row: row + 1,
                },
                shape: CursorShape::Block,
                visible: true,
            },
            flags: PaneFlags {
                prompt_row: Some(row),
                ..Default::default()
            },
            selection: Vec::new(),
        }]);
        mirror.insert_history(
            PaneId(1),
            vec![PromptSnapshot {
                row,
                id: 1,
                info: info.clone(),
                timestamp: 0,
            }],
        );
        let layout = WindowLayout {
            window: WindowId(1),
            area: CellRect {
                origin: CellPoint { col: 0, row: 0 },
                size,
            },
            panes: vec![PanePlacement {
                pane: PaneId(1),
                rect: CellRect {
                    origin: CellPoint { col: 0, row: 0 },
                    size,
                },
                band: 0,
            }],
            separators: Vec::new(),
            active: PaneId(1),
            zoomed: false,
        };
        let infos = BTreeMap::from([(PaneId(1), info.clone())]);
        let status = status_parts(&StatusLine::default());
        let mut overlays = Overlays::default();
        let input = ChromeInput {
            px: (width, height),
            scale,
            padding,
            metrics,
            ui: fonts.ui_metrics(),
            theme: &theme,
            status: &status,
            prompt: None,
            layout: Some(&layout),
            hover: None,
            overlays: None,
            notice: None,
            isles: Some(Isles {
                info: &infos,
                mirror: &mirror,
                config: &config.prompt,
                clock: None,
                date: None,
                home: Some(Path::new("/home/developer")),
            }),
        };
        let initial = chrome_layout::layout(&input, |text, bold| fonts.ui_width(text, bold));
        let anchor = initial
            .hits
            .iter()
            .find(|hit| {
                hit.target
                    == HitTarget::Prompt {
                        pane: PaneId(1),
                        row,
                        chip: Chip::Docker,
                    }
            })
            .unwrap()
            .rect;
        let mut menu = PromptMenu::new(
            PaneId(1),
            Chip::Docker,
            &info,
            anchor,
            "skynetx-prod".into(),
        );
        menu.contexts(Ok([
            "default",
            "skynetx-dev",
            "skynetx-prod",
            "skynetx-webdev",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()));
        overlays.open_prompt(menu);
        let chrome = chrome_layout::layout(
            &ChromeInput {
                overlays: Some(&overlays),
                ..input
            },
            |text, bold| fonts.ui_width(text, bold),
        );
        let panel = chrome.overlay.as_ref().unwrap().panel;
        assert!(panel.x >= 0.0 && panel.x + panel.w <= width as f32);
        assert!(panel.y >= 0.0 && panel.y + panel.h <= height as f32);
        assert_eq!(chrome.overlay.as_ref().unwrap().rows.len(), 4);
        let mut renderer = Renderer::new(gl, metrics).unwrap();
        renderer.resize((width, height));
        renderer
            .draw_frame(
                &compose_frame(&mirror, &layout, size, &theme, false),
                &chrome.chrome,
                &mut fonts,
                theme.background,
            )
            .unwrap();
        let pixels = renderer.read_pixels();
        let at = |x: u32, y: u32| {
            let index = ((height - 1 - y) * width + x) as usize * 4;
            [pixels[index], pixels[index + 1], pixels[index + 2]]
        };
        let bg = theme.background;
        assert_eq!(at(width - 2, height - 2), [bg.r, bg.g, bg.b]);
        if let Some(path) = std::env::var_os("FLAIR_PROMPT_MENU_CAPTURE") {
            fs::create_dir_all(&path).unwrap();
            let mut file =
                fs::File::create(Path::new(&path).join(format!("menu-{width}.ppm"))).unwrap();
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            for y in 0..height {
                for x in 0..width {
                    file.write_all(&at(x, y)).unwrap();
                }
            }
        }
        drop(renderer);
        drop(ctx);
    }
}
