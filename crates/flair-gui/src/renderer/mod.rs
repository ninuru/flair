use std::{error::Error, fmt};

use flair_core::theme::Rgb;

use crate::{
    cellgrid::CellGrid,
    font::{CellMetrics, Fonts},
};

pub struct Renderer {}

#[derive(Debug)]
pub struct RenderError(pub String);

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for RenderError {}

impl Renderer {
    pub fn new(gl: glow::Context, metrics: CellMetrics) -> Result<Renderer, RenderError> {
        todo!()
    }

    pub fn resize(&mut self, px: (u32, u32)) {
        todo!()
    }

    pub fn draw(
        &mut self,
        grid: &CellGrid,
        fonts: &mut Fonts,
        padding: (f32, f32),
        background: Rgb,
    ) -> Result<(), RenderError> {
        todo!()
    }

    pub fn read_pixels(&self) -> Vec<u8> {
        todo!()
    }
}
