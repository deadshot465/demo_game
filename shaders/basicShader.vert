#version 450

layout (binding = 0) uniform ModelViewProjection
{
    mat4 view;
    mat4 projection;
} mvp;

layout (binding = 2) uniform DynamicBufferObject_1
{
    mat4 model;
} dbo;

layout (binding = 3) uniform DynamicBufferObject_2
{
    mat4 model;
} dbo_2;

layout (location = 0) in vec3 inPosition;
layout (location = 1) in vec3 inNormal;
layout (location = 2) in vec2 inTexCoord;

layout (location = 1) out vec4 outNormal;
layout (location = 2) out vec2 outTexCoord;
layout (location = 3) out vec4 fragPos;

void main()
{
    vec4 position = vec4(inPosition, 1.0);
    gl_Position = mvp.projection * mvp.view * dbo.model * position;
    
    outNormal = vec4(inNormal, 0.0);
    outNormal = transpose(inverse(dbo.model)) * outNormal;
    outTexCoord = inTexCoord;
    fragPos = dbo.model * vec4(inPosition.xyz, 1.0);
}