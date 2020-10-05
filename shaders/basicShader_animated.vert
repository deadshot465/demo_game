#version 450

layout (binding = 0) uniform ModelViewProjection
{
    mat4 view;
    mat4 projection;
} mvp;

layout (std430, binding = 2) readonly buffer ModelMatrices {
    mat4 modelMatrices[];
};

layout (push_constant) uniform PushConstant
{
    uint texture_index;
    vec4 object_color;
    uint model_index;
} pco;

layout (std430, set = 1, binding = 0) readonly buffer JointMatrices {
    mat4 jointMatrices[];
};

layout (location = 0) in vec3 inPosition;
layout (location = 1) in vec3 inNormal;
layout (location = 2) in vec2 inTexCoord;
layout (location = 3) in vec4 inJoint;
layout (location = 4) in vec4 inWeight;

layout (location = 1) out vec4 outNormal;
layout (location = 2) out vec2 outTexCoord;
layout (location = 3) out vec4 fragPos;

void main()
{
    vec4 position = vec4(inPosition, 1.0);

    mat4 skinMatrix = inWeight.x * jointMatrices[int(inJoint.x)] +
        inWeight.y * jointMatrices[int(inJoint.y)] +
        inWeight.z * jointMatrices[int(inJoint.z)] +
        inWeight.w * jointMatrices[int(inJoint.w)];

    gl_Position = mvp.projection * mvp.view * modelMatrices[pco.model_index] * skinMatrix * position;
    
    outNormal = vec4(inNormal, 0.0);
    outNormal = transpose(inverse(modelMatrices[pco.model_index])) * outNormal;
    outTexCoord = inTexCoord;
    fragPos = modelMatrices[pco.model_index] * vec4(inPosition.xyz, 1.0);
}