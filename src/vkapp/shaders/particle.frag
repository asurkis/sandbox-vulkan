#version 450

layout(location = 0) in vec2 in_offset;
layout(location = 1) in vec3 in_vel;
layout(location = 0) out vec4 out_color;

void main() {
    float r2 = dot(in_offset, in_offset);
    float brightness = max(0.0, 1.0 - r2);
    vec3 vel_col = 0.5 * normalize(in_vel) + 0.5;
    out_color = vec4(vel_col * brightness, 0.0);
}
