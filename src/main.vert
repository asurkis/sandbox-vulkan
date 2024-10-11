#version 450

layout(binding = 0) uniform UniformBufferObject {
    vec2 shift;
} ubo;

layout(location = 0) in vec2 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 0) out vec3 outColor;

void main() {
    gl_Position = vec4(inPosition + ubo.shift, 0.0, 1.0);
    outColor = inColor;
}
