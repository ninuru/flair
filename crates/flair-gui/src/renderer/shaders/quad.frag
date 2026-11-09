#version 330 core
uniform sampler2D atlas;
in vec2 uv;
in vec4 tint;
in vec2 local;
flat in vec2 half_size;
flat in float radius;
flat in int mode;
out vec4 fragment;
void main() {
    vec4 texel = texture(atlas, uv);
    if (mode == 0) fragment = tint;
    else if (mode == 1) fragment = vec4(tint.rgb * texel.a, texel.a);
    else if (mode == 2) fragment = texel;
    else {
        vec2 q = abs(local) - half_size + vec2(radius);
        float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
        fragment = tint * clamp(0.5 - d, 0.0, 1.0);
    }
}
