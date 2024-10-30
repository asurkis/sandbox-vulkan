#version 450

layout(location = 0) in vec2 in_position;
layout(location = 0) out vec3 out_color;

uint z_code_3_component(uint x) {
    x = x & 0x49249249; // 0b0100_1001_0010_0100_1001_0010_0100_1001
    x = (x ^ (x >> 2)) & 0xC30C30C3; // 0b1100_0011_0000_1100_0011_0000_1100_0011
    x = (x ^ (x >> 4)) & 0x0F00F00F; // 0b0000_1111_0000_0000_1111_0000_0000_1111
    x = (x ^ (x >> 8)) & 0xFF0000FF; // 0b1111_1111_0000_0000_0000_0000_1111_1111
    x = (x ^ (x >> 16)) & 0x0000FFFF; // 0b0000_0000_0000_0000_1111_1111_1111_1111
    return x;
}

vec3 color_by_id(uint x) {
    x = ((x & 0xFFFF) << 16) | ((x >> 16) & 0xFFFF);
    x = ((x & 0xFF00FF) << 8) | ((x >> 8) & 0xFF00FF);
    x = ((x & 0xF0F0F0F) << 4) | ((x >> 4) & 0xF0F0F0F);
    x = ((x & 0x33333333) << 2) | ((x >> 2) & 0x33333333);
    x = ((x & 0x55555555) << 1) | ((x >> 1) & 0x55555555);
    ivec3 xyz = ivec3(
        z_code_3_component(x >> 2),
        z_code_3_component(x >> 3),
        z_code_3_component(x >> 4));
    return vec3(1) - vec3(xyz) / vec3(0x3FF);
}

void main() {
    gl_PointSize = 1.0;
    gl_Position = vec4(in_position, 0, 1);
    out_color = color_by_id(gl_VertexIndex);
}
