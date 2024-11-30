#version 450

layout(binding = 1) uniform sampler2D hdr_sampler;
layout(location = 0) in vec2 in_tex_coord;
layout(location = 0) out vec4 out_color;

void main() {
    vec4 sampled = texture(hdr_sampler, in_tex_coord);
    // sampled.xyz *= 0.01;
    out_color = sampled;
}
