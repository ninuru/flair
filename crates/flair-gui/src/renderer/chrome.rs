use flair_core::{
    geom::CellRect,
    theme::{Rgb, Rgba},
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub color: Rgba,
    pub width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub rect: PxRect,
    pub color: Rgba,
    pub radius: f32,
    pub border: Option<Border>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub x: f32,
    pub baseline: f32,
    pub text: String,
    pub color: Rgb,
    pub bold: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chrome {
    pub origin: (f32, f32),
    pub clip: PxRect,
    pub quads: Vec<Quad>,
    pub text: Vec<TextRun>,
    pub modal: Vec<Quad>,
    pub lifted: Option<CellRect>,
}

impl Chrome {
    pub fn cells_only(padding: (f32, f32), px: (u32, u32)) -> Chrome {
        Chrome {
            origin: padding,
            clip: PxRect {
                x: padding.0,
                y: padding.1,
                w: (px.0 as f32 - padding.0 * 2.0).max(0.0),
                h: (px.1 as f32 - padding.1 * 2.0).max(0.0),
            },
            ..Chrome::default()
        }
    }
}
