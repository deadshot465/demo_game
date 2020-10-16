#version 450

layout (binding = 0) uniform ModelViewProjection
{
    mat4 view;
    mat4 projection;
} mvp;

layout (std430, binding = 2) readonly buffer ModelMatrices {
    mat4 world_matrices[50];
    vec4 object_colors[];
};

layout (push_constant) uniform PushConstant
{
    uint texture_index;
    uint padding0;
    uint model_index;
    vec4 sky_color;
} pco;

layout (location = 0) in vec3 inPosition;
layout (location = 1) in vec3 inNormal;
layout (location = 2) in vec2 inTexCoord;

layout (location = 1) out vec4 outNormal;
layout (location = 2) out vec2 outTexCoord;
layout (location = 3) out vec4 fragPos;
layout (location = 4) out float visibility;

const float density = 0.007;
const float gradient = 1.5;

void main()
{
    vec4 worldPosition = world_matrices[pco.model_index] * vec4(inPosition, 1.0);
    vec4 positionRelativeToCamera = mvp.view * worldPosition;
    gl_Position = mvp.projection * positionRelativeToCamera;

    outNormal = vec4(inNormal, 0.0);
    outNormal = transpose(inverse(world_matrices[pco.model_index])) * outNormal;
    outTexCoord = inTexCoord;
    fragPos = worldPosition;

    float distance = length(positionRelativeToCamera.xyz);
    visibility = exp(-pow((distance * density), gradient));
    visibility = clamp(visibility, 0.0, 1.0);
}