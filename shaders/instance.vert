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
layout (location = 3) in vec3 inInstanceTranslation;
layout (location = 4) in vec3 inInstanceScale;
layout (location = 5) in vec3 inInstanceRotation;

layout (location = 1) out vec4 outNormal;
layout (location = 2) out vec2 outTexCoord;
layout (location = 3) out vec4 fragPos;
layout (location = 4) out float visibility;
layout (location = 5) out vec4 toCameraDirection;

const float density = 0.0035;
const float gradient = 5.0;

void main()
{
    mat3 mx, my, mz;
    float s = sin(inInstanceRotation.x);
    float c = cos(inInstanceRotation.x);
    mx[0] = vec3(c, s, 0.0);
    mx[1] = vec3(-s, c, 0.0);
    mx[2] = vec3(0.0, 0.0, 1.0);

    s = sin(inInstanceRotation.y);
    c = cos(inInstanceRotation.y);
    my[0] = vec3(c, 0.0, s);
    my[1] = vec3(0.0, 1.0, 0.0);
    my[2] = vec3(-s, 0.0, c);

    s = sin(inInstanceRotation.z);
    c = cos(inInstanceRotation.z);
    mz[0] = vec3(1.0, 0.0, 0.0);
    mz[1] = vec3(0.0, c, s);
    mz[2] = vec3(0.0, -s, c);

    mat3 rotation_matrix = mz * my * mx;
    vec3 local_position = rotation_matrix * inPosition;
    local_position.x *= inInstanceScale.x;
    local_position.y *= inInstanceScale.y;
    local_position.z *= inInstanceScale.z;
    vec4 position = vec4((local_position + inInstanceTranslation), 1.0);
    vec4 worldPosition = world_matrices[pco.model_index] * position;
    vec4 positionRelativeToCamera = mvp.view * worldPosition;
    gl_Position = mvp.projection * positionRelativeToCamera;

    outNormal = vec4(inNormal, 0.0);
    outNormal = transpose(inverse(mat4(rotation_matrix) * world_matrices[pco.model_index])) * outNormal;
    outTexCoord = inTexCoord;
    fragPos = worldPosition;
    toCameraDirection = (inverse(mvp.view) * vec4(0.0, 0.0, 0.0, 1.0)) - worldPosition;

    float distance = length(positionRelativeToCamera.xyz);
    visibility = exp(-pow((distance * density), gradient));
    visibility = clamp(visibility, 0.0, 1.0);
}