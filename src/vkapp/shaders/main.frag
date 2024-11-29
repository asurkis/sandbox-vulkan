#version 450

layout(location = 0) in vec3 in_norm;
layout(location = 0) out vec4 out_color;

void main() {
    float factor = gl_FragCoord.z / gl_FragCoord.w;
    if (gl_FrontFacing) {
        factor *= -1;
    }
    out_color = vec4(vec3(factor), 0);
}
