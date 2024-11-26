#version 450

layout(location = 0) in vec3 in_norm;
layout(location = 0) out vec4 out_color;

void main() {
    // out_color = vec4(0.5 * in_norm + 0.5, 1);
    out_color = vec4(vec3(4.0 / 255.0), 1.0);
}
