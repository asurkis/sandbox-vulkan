#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 mat_view;
    mat4 mat_proj;
    mat4 mat_view_proj;
} ubo;

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec2 in_texcoord;
layout(location = 0) out vec2 out_texcoord;

void main() {
    gl_Position = ubo.mat_view_proj * vec4(in_position, 1.0);
    out_texcoord = in_texcoord;
}
