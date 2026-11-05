use super::RenderError;
use glow::HasContext;

pub struct Pipeline {
    program: glow::Program,
    vao: glow::VertexArray,
    buffer: glow::Buffer,
    viewport: Option<glow::UniformLocation>,
    pub texture: glow::Texture,
}

impl Pipeline {
    pub fn new(gl: &glow::Context, atlas_size: i32) -> Result<Self, RenderError> {
        unsafe {
            let program = gl.create_program().map_err(RenderError)?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, include_str!("shaders/quad.vert")),
                (glow::FRAGMENT_SHADER, include_str!("shaders/quad.frag")),
            ] {
                let shader = gl.create_shader(kind).map_err(RenderError)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(RenderError(error));
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(RenderError(error));
            }
            let vao = gl.create_vertex_array().map_err(RenderError)?;
            let buffer = gl.create_buffer().map_err(RenderError)?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            for (index, size, offset) in [(0, 4, 0), (1, 4, 16), (2, 4, 32), (3, 1, 48)] {
                gl.enable_vertex_attrib_array(index);
                gl.vertex_attrib_pointer_f32(index, size, glow::FLOAT, false, 52, offset);
                gl.vertex_attrib_divisor(index, 1);
            }
            let texture = gl.create_texture().map_err(RenderError)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                atlas_size,
                atlas_size,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            let viewport = gl.get_uniform_location(program, "viewport");
            gl.use_program(Some(program));
            gl.uniform_1_i32(gl.get_uniform_location(program, "atlas").as_ref(), 0);
            Ok(Self {
                program,
                vao,
                buffer,
                viewport,
                texture,
            })
        }
    }
    pub fn draw(&self, gl: &glow::Context, viewport: (u32, u32), instances: &[f32]) {
        if instances.is_empty() {
            return;
        }
        unsafe {
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.uniform_2_f32(self.viewport.as_ref(), viewport.0 as f32, viewport.1 as f32);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer));
            let bytes = std::slice::from_raw_parts(
                instances.as_ptr().cast::<u8>(),
                std::mem::size_of_val(instances),
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, (instances.len() / 13) as i32);
        }
    }
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_texture(self.texture);
            gl.delete_buffer(self.buffer);
            gl.delete_vertex_array(self.vao);
            gl.delete_program(self.program);
        }
    }
}
