#version 450

layout(push_constant, std430) uniform Params {
    vec4 screen_data; // xy --- screen size, zw = 1/xy --- pixel size
    vec4 kernel_radius; // xy --- box blur kernel radius, zw --- unused
} params;

layout(binding = 1) uniform sampler2D img;

layout(location = 0) in vec2 in_tex_coord;
layout(location = 0) out vec4 out_color;

void main() {
    vec4 sampled = texture(img, in_tex_coord);
    out_color = vec4(sampled.xyz, 0);
}
