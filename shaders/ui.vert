#version 450

layout (binding = 0) uniform Locals {
    mat4 projection_matrix;
};

layout (location = 0) in vec2 inPosition;
layout (location = 1) in vec2 inUV;
layout (location = 2) in uvec4 inColor;

layout (location = 0) out vec2 outUV;
layout (location = 1) out vec4 outColor;

void main() {
    outUV = inUV;
    outColor = vec4(inColor[0] / 255.0, inColor[1] / 255.0, inColor[2] / 255.0, inColor[3] / 255.0);
    gl_Position = projection_matrix * vec4(inPosition, 0.0, 1.0);
    gl_Position.y = -gl_Position.y;
}
