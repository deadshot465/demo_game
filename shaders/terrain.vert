#version 450

layout (binding = 0) uniform ModelViewProjection
{
    mat4 view;
    mat4 projection;
} mvp;

layout (location = 0) in vec3 inPosition;
layout (location = 1) in vec3 inNormal;
layout (location = 2) in vec2 inUV;

layout (location = 0) out vec4 outNormal;
layout (location = 1) out vec2 outUV;

void main()
{
    vec4 position = vec4(inPosition, 1.0);
    gl_Position = position;
    outNormal = vec4(inNormal, 0.0);
    outUV = inUV;
}