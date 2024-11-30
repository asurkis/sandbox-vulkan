#version 450

layout(push_constant, std430) uniform Params {
    vec4 screen_data; // xy --- screen size, zw = 1/xy --- pixel size
    vec4 kernel_radius; // xy --- box blur kernel radius, zw --- unused
} params;

layout(binding = 1) uniform sampler2D img;

layout(location = 0) in vec2 in_tex_coord;
layout(location = 0) out vec4 out_color;

void main() {
    vec4 sampled = vec4(0);
    float ksum = 0;
    for (float x = -params.kernel_radius.x; x <= params.kernel_radius.x; ++x) {
        float k = exp2(-0.5 / (params.kernel_radius.x + 0.01) * x * x);
        sampled += k * texture(img, in_tex_coord + vec2(x, 0) * params.screen_data.zw);
        ksum += k;
    }
    sampled /= ksum;
    out_color = sampled;
}
