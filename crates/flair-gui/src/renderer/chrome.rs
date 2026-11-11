use flair_core::theme::{Rgb, Rgba};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFace {
    Ui,
    Small,
    Mono,
    Icon,
    PromptIcon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub x: f32,
    pub baseline: f32,
    pub text: String,
    pub color: Rgb,
    pub bold: bool,
    pub face: TextFace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chrome {
    pub scale: f32,
    pub origin: (f32, f32),
    pub clip: PxRect,
    pub extend: PxRect,
    pub quads: Vec<Quad>,
    pub text: Vec<TextRun>,
    pub modal: Vec<Quad>,
    pub modal_text: Vec<TextRun>,
}

impl Default for Chrome {
    fn default() -> Chrome {
        Chrome {
            scale: 1.0,
            origin: (0.0, 0.0),
            clip: PxRect::default(),
            extend: PxRect::default(),
            quads: Vec::new(),
            text: Vec::new(),
            modal: Vec::new(),
            modal_text: Vec::new(),
        }
    }
}
