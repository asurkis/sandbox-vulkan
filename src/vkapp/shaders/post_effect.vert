#version 450

const vec2[] positions = vec2[](vec2(-1, -1), vec2(-1, 3), vec2(3, -1));

layout(location = 0) out vec2 out_tex_coord;

void main() {
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0, 1);
    out_tex_coord = 0.5 * pos + 0.5;
}
