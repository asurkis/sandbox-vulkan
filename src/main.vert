#version 450

layout(location = 0) in vec2 in_position;

void main() {
    gl_Position = vec4(in_position, 0, 1);
    gl_PointSize = 1.0;
}
