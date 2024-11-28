#version 450

layout(binding = 1) uniform sampler2D hdr_sampler;
layout(location = 0) in vec2 in_tex_coord;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = texture(hdr_sampler, in_tex_coord);
}
