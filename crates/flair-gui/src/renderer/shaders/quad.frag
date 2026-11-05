#version 330 core
uniform sampler2D atlas;
in vec2 uv;
in vec4 tint;
flat in int mode;
out vec4 fragment;
void main() {
    vec4 texel = texture(atlas, uv);
    if (mode == 0) fragment = tint;
    else if (mode == 1) fragment = vec4(tint.rgb * texel.a, texel.a);
    else fragment = texel;
}
