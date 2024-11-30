#version 450

layout(binding = 0, std140) uniform CameraBuffer {
    mat4 mat_view;
    mat4 mat_proj;
    mat4 mat_view_proj;
} cam;

layout(location = 0) in vec4 in_pos;
layout(location = 1) in vec4 in_vel;
layout(location = 0) out vec2 out_offset;
layout(location = 1) out vec3 out_vel;

const vec2 offsets[] = vec2[](vec2(-1, -1), vec2(-1, 1), vec2(1, -1), vec2(1, 1));

void main() {
    vec4 pos_view = cam.mat_view * vec4(in_pos.xyz, 1);
    out_offset = offsets[gl_VertexIndex];
    pos_view.xy += 0.125 * out_offset;
    gl_Position = cam.mat_proj * pos_view;
    out_vel = in_vel.xyz;
}
