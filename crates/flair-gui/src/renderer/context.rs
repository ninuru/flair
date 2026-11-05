use super::RenderError;
use glow::HasContext;
use glutin::{
    api::egl::{
        context::PossiblyCurrentContext, device::Device, display::Display, surface::Surface,
    },
    config::{Api, ConfigSurfaceTypes, ConfigTemplateBuilder},
    context::{ContextApi, ContextAttributesBuilder, GlProfile, Version},
    prelude::*,
    surface::{SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use std::{ffi::CString, num::NonZeroU32};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

pub struct WindowContext {
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
}

pub struct HeadlessContext {
    gl: glow::Context,
    framebuffer: glow::Framebuffer,
    texture: glow::Texture,
    context: PossiblyCurrentContext,
}

fn error(e: impl std::fmt::Display) -> RenderError {
    RenderError(e.to_string())
}
fn attributes(
    window: Option<winit::raw_window_handle::RawWindowHandle>,
) -> glutin::context::ContextAttributes {
    ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .with_profile(GlProfile::Core)
        .build(window)
}
fn load_gl(display: &Display) -> glow::Context {
    unsafe {
        glow::Context::from_loader_function(|name| {
            display.get_proc_address(&CString::new(name).unwrap())
        })
    }
}

impl WindowContext {
    pub fn new(window: &Window) -> Result<(WindowContext, glow::Context), RenderError> {
        let raw_display = window.display_handle().map_err(error)?.as_raw();
        let raw_window = window.window_handle().map_err(error)?.as_raw();
        let display = unsafe { Display::new(raw_display).map_err(error)? };
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_api(Api::OPENGL)
            .build();
        let config = unsafe {
            display
                .find_configs(template)
                .map_err(error)?
                .next()
                .ok_or_else(|| error("no EGL window config"))?
        };
        let size = window.inner_size();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        let surface = unsafe {
            display
                .create_window_surface(&config, &attrs)
                .map_err(error)?
        };
        let context = unsafe {
            display
                .create_context(&config, &attributes(Some(raw_window)))
                .map_err(error)?
        }
        .make_current(&surface)
        .map_err(error)?;
        surface
            .set_swap_interval(&context, SwapInterval::DontWait)
            .map_err(error)?;
        let gl = load_gl(&display);
        Ok((Self { surface, context }, gl))
    }
    pub fn resize(&self, px: (u32, u32)) {
        self.surface.resize(
            &self.context,
            NonZeroU32::new(px.0.max(1)).unwrap(),
            NonZeroU32::new(px.1.max(1)).unwrap(),
        );
    }
    pub fn swap_buffers(&self) -> Result<(), RenderError> {
        self.surface.swap_buffers(&self.context).map_err(error)
    }
}

impl HeadlessContext {
    pub fn new(px: (u32, u32)) -> Result<(HeadlessContext, glow::Context), RenderError> {
        if px.0 == 0 || px.1 == 0 {
            return Err(error("headless dimensions must be positive"));
        }
        let mut last = error("no EGL device");
        for device in Device::query_devices().map_err(error)? {
            match Self::for_device(&device, px) {
                Ok(pair) => return Ok(pair),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
    fn for_device(device: &Device, px: (u32, u32)) -> Result<(Self, glow::Context), RenderError> {
        let display = unsafe { Display::with_device(device, None).map_err(error)? };
        let template = ConfigTemplateBuilder::new()
            .with_surface_type(ConfigSurfaceTypes::PBUFFER)
            .with_api(Api::OPENGL)
            .build();
        let config = unsafe {
            display
                .find_configs(template)
                .map_err(error)?
                .next()
                .ok_or_else(|| error("no headless EGL config"))?
        };
        let context = unsafe {
            display
                .create_context(&config, &attributes(None))
                .map_err(error)?
        }
        .make_current_surfaceless()
        .map_err(error)?;
        let gl = load_gl(&display);
        let (framebuffer, texture) = unsafe {
            let framebuffer = gl.create_framebuffer().map_err(error)?;
            let texture = gl.create_texture().map_err(error)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                px.0 as i32,
                px.1 as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                gl.delete_texture(texture);
                gl.delete_framebuffer(framebuffer);
                return Err(error("headless framebuffer incomplete"));
            }
            (framebuffer, texture)
        };
        let renderer_gl = load_gl(&display);
        Ok((
            Self {
                gl,
                framebuffer,
                texture,
                context,
            },
            renderer_gl,
        ))
    }
}

impl Drop for HeadlessContext {
    fn drop(&mut self) {
        if self.context.make_current_surfaceless().is_ok() {
            unsafe {
                self.gl.delete_framebuffer(self.framebuffer);
                self.gl.delete_texture(self.texture);
            }
        }
    }
}
