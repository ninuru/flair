use crate::{
    chrome_bar::{baseline, ellipsize, opaque, rounded, ui_text},
    chrome_layout::bar_height,
    font::PROMPT_ICON_SCALE,
    overlay_layout::{OverlayInput, OverlayScene},
    renderer::{Border, PxRect, TextFace, TextRun},
    t3_style::{
        FOREGROUND, MUTED, POPUP, POPUP_BORDER, POPUP_HOVER, POPUP_PAD, POPUP_ROW_RADIUS,
        POPUP_SELECTED, SELECT_RADIUS,
    },
    ui::{LINE, px},
};

use super::PromptMenu;

impl PromptMenu {
    pub fn layout(
        &self,
        input: &OverlayInput,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) -> OverlayScene {
        let mut scene = OverlayScene::default();
        let scale = input.scale;
        let inset = px(8.0, scale);
        let row_h = px(28.0, scale)
            .max(input.ui.height + px(8.0, scale))
            .max(input.metrics.height + px(4.0, scale));
        let (field_h, foot, gap) = match self.chip {
            Some(_) => (row_h, 0.0, px(4.0, scale)),
            None => (px(POPUP_PAD, scale), px(POPUP_PAD, scale), 0.0),
        };
        let rows = self.filtered();
        let top = bar_height(scale) + inset;
        let available = (input.px.1 as f32 - top - inset).max(0.0);
        let capacity = (((available - field_h - foot) / row_h).floor().max(0.0) as usize).min(8);
        let count = rows.len().max(1).min(capacity);
        let height = (field_h + count as f32 * row_h + foot).min(available);
        let width = px(if self.chip.is_some() { 360.0 } else { 220.0 }, scale)
            .min((input.px.0 as f32 - inset * 2.0).max(0.0));
        let left = self
            .anchor
            .x
            .min(input.px.0 as f32 - inset - width)
            .max(inset);
        let below = self.anchor.y + self.anchor.h + gap;
        let y = if below + height <= input.px.1 as f32 - inset {
            below
        } else {
            (self.anchor.y - gap - height).max(top)
        };
        let panel = PxRect {
            x: left,
            y,
            w: width,
            h: height,
        };
        scene.hits.panel = panel;
        scene.quads.push(crate::renderer::Quad {
            border: Some(Border {
                color: opaque(POPUP_BORDER),
                width: px(LINE, scale),
            }),
            ..rounded(panel, POPUP, SELECT_RADIUS, scale)
        });
        if height < field_h || width <= inset * 2.0 {
            return scene;
        }
        if self.chip.is_some() {
            let field = PxRect {
                h: field_h,
                ..panel
            };
            let placeholder = if self.chip == Some(flair_core::isles::Chip::Docker) {
                "Search Docker contexts…"
            } else {
                "Search actions…"
            };
            let text = if self.query.is_empty() {
                self.error.as_deref().unwrap_or(placeholder)
            } else {
                &self.query
            };
            scene.text.push(ui_text(
                left + inset,
                baseline(field, input.ui),
                ellipsize(text, width - inset * 2.0, false, measure),
                if self.query.is_empty() {
                    MUTED
                } else {
                    FOREGROUND
                },
                false,
            ));
        }
        let first = self.selected.saturating_sub(capacity.saturating_sub(1));
        for (offset, item) in rows.iter().skip(first).take(count).enumerate() {
            let index = first + offset;
            let rect = PxRect {
                x: left,
                y: y + field_h + offset as f32 * row_h,
                w: width,
                h: row_h,
            };
            let hovered = input.hover.is_some_and(|(x, y)| {
                x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
            });
            let selected = index == self.selected;
            if selected || hovered {
                let pad = px(POPUP_PAD, scale);
                scene.quads.push(rounded(
                    PxRect {
                        x: rect.x + pad,
                        w: (rect.w - pad * 2.0).max(0.0),
                        ..rect
                    },
                    if selected {
                        POPUP_SELECTED
                    } else {
                        POPUP_HOVER
                    },
                    POPUP_ROW_RADIUS,
                    scale,
                ));
            }
            if self.chip == Some(flair_core::isles::Chip::Docker) {
                scene.text.push(TextRun {
                    x: left + inset,
                    baseline: (rect.y
                        + (rect.h - input.metrics.height * PROMPT_ICON_SCALE) / 2.0
                        + input.metrics.baseline * PROMPT_ICON_SCALE)
                        .round(),
                    text: "\u{f308}".into(),
                    color: FOREGROUND,
                    bold: false,
                    face: TextFace::PromptIcon,
                });
            }
            if item.current {
                scene.text.push(ui_text(
                    left + width - px(24.0, scale),
                    baseline(rect, input.ui),
                    "✓".into(),
                    MUTED,
                    false,
                ));
            }
            let label = px(if self.chip.is_some() { 28.0 } else { 16.0 }, scale);
            scene.text.push(ui_text(
                left + label,
                baseline(rect, input.ui),
                ellipsize(&item.name, width - label * 2.0, false, measure),
                FOREGROUND,
                false,
            ));
            scene.hits.rows.push((index, rect));
        }
        if rows.is_empty() && count > 0 {
            let text = if self.loading {
                "Loading Docker contexts…"
            } else if let Some(error) = &self.error {
                error
            } else {
                "No matches"
            };
            let rect = PxRect {
                y: y + field_h,
                h: row_h,
                ..panel
            };
            scene.text.push(ui_text(
                left + inset,
                baseline(rect, input.ui),
                ellipsize(text, width - inset * 2.0, false, measure),
                MUTED,
                false,
            ));
        }
        scene
    }
}
