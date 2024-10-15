#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 mat_view;
    mat4 mat_proj;
    mat4 mat_view_proj;
} ubo;

layout(location = 0) in vec3 inPosition;
layout(location = 0) out vec3 outColor;

void main() {
    gl_Position = ubo.mat_view_proj * vec4(inPosition, 1.0);
    outColor = 0.5 * (inPosition + 1.0);
}
