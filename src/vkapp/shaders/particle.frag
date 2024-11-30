#version 450

layout(location = 0) in vec2 in_offset;
layout(location = 1) in vec4 in_color_tint;
layout(location = 0) out vec4 out_color;

void main() {
    float r2 = dot(in_offset, in_offset);
    float brightness = smoothstep(0.0, 1.0, 1.0 - r2);
    vec3 tint = in_color_tint.w * (0.5 * normalize(in_color_tint.xyz) + 0.5);
    out_color = vec4(tint * brightness, 0.0);
}
