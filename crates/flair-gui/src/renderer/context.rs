use winit::window::Window;

use super::RenderError;

pub struct WindowContext {}

pub struct HeadlessContext {}

impl WindowContext {
    #[allow(unused_variables)]
    pub fn new(window: &Window) -> Result<(WindowContext, glow::Context), RenderError> {
        todo!()
    }

    #[allow(unused_variables)]
    pub fn resize(&self, px: (u32, u32)) {
        todo!()
    }

    pub fn swap_buffers(&self) -> Result<(), RenderError> {
        todo!()
    }
}

impl HeadlessContext {
    #[allow(unused_variables)]
    pub fn new(px: (u32, u32)) -> Result<(HeadlessContext, glow::Context), RenderError> {
        todo!()
    }
}
