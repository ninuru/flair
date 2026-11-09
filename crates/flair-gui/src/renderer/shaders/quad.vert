#version 330 core
layout(location = 0) in vec4 rect;
layout(location = 1) in vec4 texture_rect;
layout(location = 2) in vec4 color;
layout(location = 3) in float kind;
uniform vec2 viewport;
out vec2 uv;
out vec4 tint;
out vec2 local;
flat out vec2 half_size;
flat out float radius;
flat out int mode;
const vec2 corners[6] = vec2[6](vec2(0,0),vec2(1,0),vec2(0,1),vec2(0,1),vec2(1,0),vec2(1,1));
void main() {
    vec2 corner = corners[gl_VertexID];
    vec2 pixel = rect.xy + corner * rect.zw;
    gl_Position = vec4(pixel.x / viewport.x * 2.0 - 1.0, 1.0 - pixel.y / viewport.y * 2.0, 0.0, 1.0);
    uv = texture_rect.xy + corner * texture_rect.zw;
    tint = color;
    half_size = rect.zw * 0.5;
    local = corner * rect.zw - half_size;
    radius = texture_rect.x;
    mode = int(kind);
}
