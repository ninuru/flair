use super::{PromptAction, PromptEditor};
use crate::{
    CellMetrics,
    chrome_bar::{ellipsize, opaque, rounded, ui_text},
    overlay_layout::{OverlayInput, OverlayScene},
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    settings_layout::{self, SettingsHit, SettingsSheet},
    ui::{
        GROUP_GAP, ITEM_GAP, ITEM_PAD, ROW_GAP, ROW_HEIGHT, ROW_RADIUS, SWATCH, SWATCH_RADIUS, px,
    },
};
use flair_core::{
    config::{
        AnsiColour, PromptAppearance, PromptDateFormat, PromptPath, PromptSeparator, PromptStyle,
        PromptTimeFormat,
    },
    isles::{AheadBehind, Changes, Chip, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    theme::Rgb,
    time::Millis,
};

#[derive(Clone, Copy)]
enum Emphasis {
    Control,
    Quiet,
    Tray,
    Selected,
    Primary,
}

struct Button {
    action: PromptAction,
    emphasis: Emphasis,
    enabled: bool,
    swatch: Option<Rgb>,
}

impl PromptEditor {
    pub(crate) fn layout(
        &self,
        sheet: &SettingsSheet,
        input: &OverlayInput,
        metrics: CellMetrics,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) -> OverlayScene {
        let mut sheet = sheet.clone();
        sheet.groups.clear();
        sheet.focus = None;
        sheet.keys = vec![
            ("Space", "toggle".into()),
            ("Shift ←/→", "reorder".into()),
            ("Enter", "save".into()),
            ("Esc", "cancel".into()),
        ];
        let mut scene = settings_layout::layout(&sheet, input, measure);
        let rect = settings_layout::editor_rect(input);
        let scale = input.scale;
        if rect.w < px(160.0, scale) || rect.h < px(100.0, scale) {
            return scene;
        }
        let preview = PxRect {
            h: px(80.0, scale),
            ..rect
        };
        self.preview(preview, input, metrics, &mut scene, measure);
        let tray = PxRect {
            y: preview.y + preview.h + px(12.0, scale),
            ..rect
        };
        let top = self.tray(tray, input, &mut scene, measure) + px(16.0, scale);
        let columns = if rect.w >= px(520.0, scale) { 3 } else { 2 };
        let selected = self.selected_controls();
        let general = self.general_controls();
        let heading = px(28.0, scale);
        let row = px(ROW_HEIGHT + ROW_GAP, scale);
        let selected_height = heading + selected.len().div_ceil(columns) as f32 * row;
        let general_height =
            heading + general.len().div_ceil(columns) as f32 * row + px(24.0, scale);
        let content_height = selected_height + px(GROUP_GAP, scale) + general_height;
        let end = rect.y + rect.h;
        let actions_y = (top + content_height + px(28.0, scale)).min(end - px(32.0, scale));
        let bottom = actions_y - px(28.0, scale);
        let overflow = (content_height - (bottom - top)).max(0.0);
        let scroll_step = px(4.0, scale).max(1.0);
        let limit = (overflow / scroll_step).ceil() as usize;
        scene.hits.prompt_scroll_limit = Some(limit);
        let offset = (self.scroll.min(limit) as f32 * scroll_step).min(overflow);
        let viewport = PxRect {
            y: top,
            h: (bottom - top).max(0.0),
            ..rect
        };
        let selected_top = top - offset;
        self.group(
            title(self.selected),
            &selected,
            (selected_top, viewport),
            input,
            &mut scene,
            measure,
        );
        let general_top = selected_top + selected_height + px(GROUP_GAP, scale);
        self.group(
            "Prompt",
            &general,
            (general_top, viewport),
            input,
            &mut scene,
            measure,
        );
        let note_y = general_top + general_height - px(8.0, scale);
        if note_y >= top && note_y <= bottom {
            label(
                "Input separator and shell prompt apply to new shells.",
                (rect.x, note_y, rect.w),
                false,
                input,
                &mut scene,
                measure,
            );
        }
        if overflow > 0.0 {
            let hint = if offset + 0.5 < overflow {
                "Scroll for more options ↓"
            } else {
                "Scroll to return to segment options ↑"
            };
            label(
                hint,
                (rect.x, actions_y - px(10.0, scale), rect.w),
                false,
                input,
                &mut scene,
                measure,
            );
        }
        self.actions(
            PxRect {
                y: actions_y,
                h: px(32.0, scale),
                ..rect
            },
            input,
            &mut scene,
            measure,
        );
        scene
    }

    fn preview(
        &self,
        rect: PxRect,
        input: &OverlayInput,
        metrics: CellMetrics,
        scene: &mut OverlayScene,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) {
        let scale = input.scale;
        scene.quads.push(rounded(
            rect,
            input.theme.surfaces().surface1,
            ROW_RADIUS,
            scale,
        ));
        let inset = px(12.0, scale);
        let row = PxRect {
            x: rect.x + inset,
            y: rect.y + inset,
            w: (rect.w - inset * 2.0).max(0.0),
            h: metrics.height + px(4.0, scale),
        };
        let hits = crate::isles::preview(input, metrics, &self.draft, &sample(), row, scene);
        if self.draft.style == PromptStyle::Ps1 {
            label(
                "Your shell’s prompt (PS1)",
                (row.x, row.y + input.ui.ascent, row.w),
                false,
                input,
                scene,
                measure,
            );
        } else if hits.is_empty() {
            label(
                "No segments selected — add one below",
                (row.x, row.y + input.ui.ascent, row.w),
                false,
                input,
                scene,
                measure,
            );
        }
        scene.hits.controls.extend(
            hits.into_iter()
                .map(|(chip, rect)| (SettingsHit::PromptEditor(PromptAction::Select(chip)), rect)),
        );
        let prefix = match self.draft.separator {
            PromptSeparator::None => "",
            PromptSeparator::Chevron => "❯ ",
            PromptSeparator::Dollar => "$ ",
            PromptSeparator::Percent => "% ",
        };
        scene.text.push(TextRun {
            x: row.x,
            baseline: (rect.y + rect.h - inset).round(),
            text: format!("{prefix}cargo build"),
            face: TextFace::Mono,
            color: input.theme.foreground,
            bold: false,
        });
    }

    fn tray(
        &self,
        rect: PxRect,
        input: &OverlayInput,
        scene: &mut OverlayScene,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) -> f32 {
        let scale = input.scale;
        let gap = px(ITEM_GAP, scale);
        let mut x = rect.x;
        let mut y = rect.y;
        for chip in self.ordered() {
            let shown = self.draft.enabled(chip);
            let text = format!("{} {}", if shown { "✓" } else { "+" }, title(chip));
            let width = (measure(&text, false) + px(ITEM_PAD * 2.0, scale))
                .ceil()
                .min(rect.w);
            if x > rect.x && x + width > rect.x + rect.w {
                x = rect.x;
                y += px(ROW_HEIGHT, scale);
            }
            let at = PxRect {
                x,
                y,
                w: width,
                h: px(26.0, scale),
            };
            let action = if shown {
                PromptAction::Select(chip)
            } else {
                PromptAction::Add(chip)
            };
            let emphasis = if chip == self.selected {
                Emphasis::Selected
            } else {
                Emphasis::Tray
            };
            button(
                &text,
                Button {
                    action,
                    emphasis,
                    enabled: true,
                    swatch: None,
                },
                at,
                input,
                scene,
                measure,
            );
            x += width + gap;
        }
        y + px(26.0, scale)
    }

    fn selected_controls(&self) -> Vec<(String, PromptAction)> {
        let segment = self
            .draft
            .segments
            .get(&self.selected)
            .copied()
            .unwrap_or_default();
        vec![
            (
                format!("Colour: {}", segment.colour.map_or("Theme", colour_name)),
                PromptAction::Colour,
            ),
            (
                format!(
                    "Appearance: {}",
                    segment.appearance.map_or("Default", appearance_name)
                ),
                PromptAction::Appearance,
            ),
            (
                format!(
                    "Icon: {}",
                    segment
                        .icons
                        .map_or("Default", |value| if value { "On" } else { "Off" })
                ),
                PromptAction::Icon,
            ),
            ("Move left".into(), PromptAction::Move(-1)),
            ("Move right".into(), PromptAction::Move(1)),
            (
                if self.draft.enabled(self.selected) {
                    "Remove segment".into()
                } else {
                    "Add segment".into()
                },
                PromptAction::Toggle,
            ),
        ]
    }

    fn general_controls(&self) -> Vec<(String, PromptAction)> {
        vec![
            (
                format!(
                    "Path: {}",
                    match self.draft.path {
                        PromptPath::Full => "Full path",
                        PromptPath::Abbreviated => "Home as ~",
                        PromptPath::Folder => "Folder only",
                    }
                ),
                PromptAction::Path,
            ),
            (
                format!(
                    "Clock: {}",
                    match self.draft.time_format {
                        PromptTimeFormat::TwentyFour => "24-hour",
                        PromptTimeFormat::Twelve => "12-hour",
                    }
                ),
                PromptAction::Time,
            ),
            (
                format!(
                    "Date: {}",
                    match self.draft.date_format {
                        PromptDateFormat::Iso => "YYYY-MM-DD",
                        PromptDateFormat::Short => "MM/DD/YYYY",
                        PromptDateFormat::Long => "Month D, YYYY",
                    }
                ),
                PromptAction::Date,
            ),
            (
                format!(
                    "Input: {}",
                    match self.draft.separator {
                        PromptSeparator::None => "Plain",
                        PromptSeparator::Chevron => "Chevron ❯",
                        PromptSeparator::Dollar => "Dollar $",
                        PromptSeparator::Percent => "Percent %",
                    }
                ),
                PromptAction::Separator,
            ),
            (
                format!("Appearance: {}", appearance_name(self.draft.appearance)),
                PromptAction::GlobalAppearance,
            ),
            (
                format!("Icons: {}", if self.draft.icons { "On" } else { "Off" }),
                PromptAction::GlobalIcons,
            ),
            (
                format!(
                    "Prompt: {}",
                    match self.draft.style {
                        PromptStyle::Chips => "Chips",
                        PromptStyle::Ps1 => "Shell (PS1)",
                    }
                ),
                PromptAction::Style,
            ),
            (
                format!("Duration: {} ms −", self.draft.duration_ms),
                PromptAction::Duration(-1),
            ),
            ("Duration +500 ms".into(), PromptAction::Duration(1)),
        ]
    }

    fn group(
        &self,
        heading: &str,
        controls: &[(String, PromptAction)],
        bounds: (f32, PxRect),
        input: &OverlayInput,
        scene: &mut OverlayScene,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) {
        let scale = input.scale;
        let (y, viewport) = bounds;
        let height = px(28.0, scale);
        let bottom = viewport.y + viewport.h;
        if y >= viewport.y && y + height <= bottom {
            label(
                heading,
                (viewport.x, y + input.ui.ascent, viewport.w),
                true,
                input,
                scene,
                measure,
            );
        }
        let columns = if viewport.w >= px(520.0, scale) { 3 } else { 2 };
        let gap = px(ITEM_GAP, scale);
        let width = ((viewport.w - gap * (columns - 1) as f32) / columns as f32).floor();
        for (index, (text, action)) in controls.iter().enumerate() {
            let rect = PxRect {
                x: viewport.x + (index % columns) as f32 * (width + gap),
                y: y + height + (index / columns) as f32 * px(ROW_HEIGHT + ROW_GAP, scale),
                w: width,
                h: px(28.0, scale),
            };
            if rect.y < viewport.y || rect.y + rect.h > bottom {
                continue;
            }
            let swatch = if *action == PromptAction::Colour {
                self.draft
                    .segments
                    .get(&self.selected)
                    .and_then(|segment| segment.colour)
                    .map(|colour| input.theme.normal[colour.index()])
            } else {
                None
            };
            button(
                text,
                Button {
                    action: *action,
                    emphasis: Emphasis::Control,
                    enabled: self.available(*action),
                    swatch,
                },
                rect,
                input,
                scene,
                measure,
            );
        }
    }

    fn actions(
        &self,
        rect: PxRect,
        input: &OverlayInput,
        scene: &mut OverlayScene,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) {
        let scale = input.scale;
        let gap = px(ITEM_GAP, scale);
        let slot = ((rect.w - gap * 2.0) / 3.0).floor();
        let save_width = px(136.0, scale).min(slot);
        let cancel_width = px(88.0, scale).min(slot);
        let reset = PxRect {
            w: px(160.0, scale).min(slot),
            ..rect
        };
        let save = PxRect {
            x: rect.x + rect.w - save_width,
            w: save_width,
            ..rect
        };
        let cancel = PxRect {
            x: save.x - gap - cancel_width,
            w: cancel_width,
            ..rect
        };
        for (text, action, at, emphasis) in [
            (
                "Restore defaults",
                PromptAction::Reset,
                reset,
                Emphasis::Quiet,
            ),
            ("Cancel", PromptAction::Cancel, cancel, Emphasis::Control),
            (
                if self.dirty() {
                    "Save changes"
                } else {
                    "Saved"
                },
                PromptAction::Save,
                save,
                Emphasis::Primary,
            ),
        ] {
            button(
                text,
                Button {
                    action,
                    emphasis,
                    enabled: self.available(action),
                    swatch: None,
                },
                at,
                input,
                scene,
                measure,
            );
        }
    }
}

fn button(
    text: &str,
    spec: Button,
    rect: PxRect,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    if rect.y < 0.0 || rect.y + rect.h > input.px.1 as f32 || rect.w <= 0.0 {
        return;
    }
    let surfaces = input.theme.surfaces();
    let hover = spec.enabled
        && input.hover.is_some_and(|(x, y)| {
            x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
        });
    let (fill, ink, border) = if !spec.enabled {
        (surfaces.surface1, surfaces.faint, None)
    } else {
        match spec.emphasis {
            Emphasis::Primary => (
                if hover {
                    input.theme.foreground
                } else {
                    surfaces.bright
                },
                input.theme.background,
                None,
            ),
            Emphasis::Selected => (
                surfaces.surface3,
                input.theme.foreground,
                Some(input.theme.accent),
            ),
            Emphasis::Control => (
                if hover {
                    surfaces.surface3
                } else {
                    surfaces.surface2
                },
                input.theme.foreground,
                Some(surfaces.outline_strong),
            ),
            Emphasis::Tray => (
                if hover {
                    surfaces.surface2
                } else {
                    surfaces.surface1
                },
                if hover {
                    input.theme.foreground
                } else {
                    surfaces.muted
                },
                Some(surfaces.outline),
            ),
            Emphasis::Quiet => (
                if hover {
                    surfaces.surface2
                } else {
                    input.theme.background
                },
                if hover {
                    input.theme.foreground
                } else {
                    surfaces.muted
                },
                None,
            ),
        }
    };
    scene.quads.push(Quad {
        border: border.map(|color| Border {
            color: opaque(color),
            width: px(1.0, input.scale),
        }),
        ..rounded(rect, fill, ROW_RADIUS, input.scale)
    });
    let mut x = rect.x + px(ITEM_PAD, input.scale);
    if let Some(colour) = spec.swatch {
        let size = px(SWATCH, input.scale);
        let swatch = PxRect {
            x,
            y: (rect.y + (rect.h - size) / 2.0).round(),
            w: size,
            h: size,
        };
        scene.quads.push(Quad {
            border: Some(Border {
                color: opaque(surfaces.outline_strong),
                width: px(1.0, input.scale),
            }),
            ..rounded(swatch, colour, SWATCH_RADIUS, input.scale)
        });
        x += size + px(ITEM_GAP, input.scale);
    }
    let text = ellipsize(
        text,
        rect.x + rect.w - px(ITEM_PAD, input.scale) - x,
        false,
        measure,
    );
    scene.text.push(ui_text(
        x,
        (rect.y + (rect.h - input.ui.height) / 2.0 + input.ui.ascent).round(),
        text,
        ink,
        false,
    ));
    if spec.enabled {
        scene
            .hits
            .controls
            .push((SettingsHit::PromptEditor(spec.action), rect));
    }
}

fn label(
    text: &str,
    at: (f32, f32, f32),
    bold: bool,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    let (x, baseline, room) = at;
    let text = ellipsize(text, room, bold, measure);
    scene.text.push(ui_text(
        x.round(),
        baseline.round(),
        text,
        if bold {
            input.theme.foreground
        } else {
            input.theme.surfaces().muted
        },
        bold,
    ));
}

fn appearance_name(value: PromptAppearance) -> &'static str {
    match value {
        PromptAppearance::Outlined => "Outlined",
        PromptAppearance::Filled => "Filled",
        PromptAppearance::Plain => "Plain",
    }
}
fn colour_name(value: AnsiColour) -> &'static str {
    match value {
        AnsiColour::Black => "Black",
        AnsiColour::Red => "Red",
        AnsiColour::Green => "Green",
        AnsiColour::Yellow => "Yellow",
        AnsiColour::Blue => "Blue",
        AnsiColour::Magenta => "Magenta",
        AnsiColour::Cyan => "Cyan",
        AnsiColour::White => "White",
    }
}
fn title(chip: Chip) -> &'static str {
    match chip {
        Chip::User => "User",
        Chip::Host => "Host",
        Chip::Cwd => "Directory",
        Chip::GitBranch => "Branch",
        Chip::GitAheadBehind => "Ahead/behind",
        Chip::GitChanges => "Changes",
        Chip::Venv => "Environment",
        Chip::Docker => "Docker",
        Chip::Kube => "Kubernetes",
        Chip::Exit => "Exit / duration",
        Chip::Time => "Time",
        Chip::Date => "Date",
    }
}

fn sample() -> PromptInfo {
    PromptInfo {
        cwd: Some("/home/developer/projects/flair".into()),
        git: Some(GitInfo {
            head: Head::Branch("main".into()),
            upstream: Some(AheadBehind {
                ahead: 1,
                behind: 2,
            }),
            changes: Some(Changes {
                files: 2,
                added: 12,
                removed: 3,
            }),
        }),
        last: Some(LastCommand {
            exit: Some(0),
            duration: Millis(2400),
        }),
        docker: Some("skynetx-prod".into()),
        kube: Some("production".into()),
        shell: Some(ShellFacts {
            user: "developer".into(),
            host: "workstation".into(),
            venv: Some(".venv".into()),
        }),
    }
}
