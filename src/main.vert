#version 450

layout(binding = 0, std140) uniform UniformBufferObject {
    mat4 mat_view;
    mat4 mat_proj;
    mat4 mat_view_proj;
} ubo;

layout(location = 0) out vec3 out_cam_pos;
layout(location = 1) out vec3 out_ray_dir;

const vec2[] positions = vec2[](vec2(-1, -1), vec2(-1, 3), vec2(3, -1));

void main() {
    vec2 screen_pos = positions[gl_VertexIndex];
    gl_Position = vec4(screen_pos, 0, 1);

    mat4 mat_view_t = transpose(ubo.mat_view);
    vec3 cam_right = mat_view_t[0].xyz;
    vec3 cam_down = mat_view_t[1].xyz;
    vec3 cam_forward = mat_view_t[2].xyz;
    out_cam_pos = (mat_view_t * vec4(-ubo.mat_view[3].xyz, 0)).xyz;

    out_ray_dir = cam_forward
        + screen_pos.x * cam_right / ubo.mat_proj[0][0]
        + screen_pos.y * cam_down / ubo.mat_proj[1][1];
}
