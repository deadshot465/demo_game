#version 450

layout (binding = 0) uniform ModelViewProjection
{
    mat4 view;
    mat4 projection;
} mvp;

layout (std430, binding = 2) readonly buffer ModelMatrices {
    mat4 world_matrices[50];
    vec4 object_colors[50];
    float reflectivities[50];
    float shine_dampers[];
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

layout (location = 1) out vec3 outNormal;
layout (location = 2) out vec2 outTexCoord;
layout (location = 3) out vec3 fragPos;
layout (location = 4) out float visibility;
layout (location = 5) out vec3 toCameraDirection;

const float density = 0.0035;
const float gradient = 5.0;

void main()
{
    vec4 worldPosition = world_matrices[pco.model_index] * vec4(inPosition, 1.0);
    vec4 positionRelativeToCamera = mvp.view * worldPosition;
    gl_Position = mvp.projection * positionRelativeToCamera;

    outNormal = inNormal;
    outNormal = mat3(transpose(inverse(world_matrices[pco.model_index]))) * outNormal;
    outTexCoord = inTexCoord * 40.0;
    fragPos = vec3(worldPosition);
    toCameraDirection = (inverse(mvp.view) * vec4(0.0, 0.0, 0.0, 1.0)).xyz - worldPosition.xyz;

    float distance = length(positionRelativeToCamera.xyz);
    visibility = exp(-pow((distance * density), gradient));
    visibility = clamp(visibility, 0.0, 1.0);
}