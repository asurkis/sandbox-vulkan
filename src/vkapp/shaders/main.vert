#version 450

layout(binding = 0, std140) uniform UniformBufferObject {
    mat4 mat_view;
    mat4 mat_proj;
    mat4 mat_view_proj;
} ubo;

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_norm;
layout(location = 0) out vec3 out_norm;

void main() {
    gl_Position = ubo.mat_view_proj * vec4(in_pos, 1);
    out_norm = in_norm;
}
