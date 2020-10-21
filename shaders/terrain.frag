#version 450
#extension GL_EXT_nonuniform_qualifier : require

layout (binding = 1) uniform DirectionalLight
{
    vec4 diffuse;
    vec3 light_position;
    float padding0;
} directional_light;

layout (std430, binding = 2) readonly buffer ModelMatrices {
    mat4 world_matrices[50];
    vec4 object_colors[50];
    float reflectivities[50];
    float shine_dampers[];
};

layout (binding = 3) uniform sampler2D tex_sampler[];

layout (push_constant) uniform PushConstant
{
    uint texture_index;
    uint padding0;
    uint model_index;
    vec4 sky_color;
} pco;

layout (location = 1) in vec3 inNormal;
layout (location = 2) in vec2 inTexCoord;
layout (location = 3) in vec3 fragPos;
layout (location = 4) in float visibility;
layout (location = 5) in vec3 toCameraDirection;

layout (location = 0) out vec4 fragColor;

const float ambientIntensity = 0.5;

void main()
{
    // Texture
    vec4 tex_color = texture(tex_sampler[pco.texture_index], inTexCoord);
    if (tex_color.a < 0.1) {
        discard;
    }

    // Ambient
    vec4 ambient = ambientIntensity * tex_color;

    // Diffuse Light
    // Pointing from the pixel to the light
    vec3 lightDirection = directional_light.light_position - fragPos;
    lightDirection = normalize(lightDirection);
    vec3 normal = normalize(inNormal);
    float diffuseIntensity = max(dot(normal, lightDirection), 0.0);
    vec4 diffuse = directional_light.diffuse * diffuseIntensity * tex_color;

    // Specular Lighting
    //vec4 normalizedToCameraDirection = normalize(toCameraDirection);
    // Pointing from the light to the surface
    /*vec4 incomingLightDirection = -lightDirection;
    vec4 reflectedLightDirection = reflect(incomingLightDirection, normal);
    float specularFactor = dot(reflectedLightDirection, normalizedToCameraDirection);
    specularFactor = max(specularFactor, 0.0);
    float dampedSpecular = pow(specularFactor, shine_dampers[pco.model_index]);
    vec4 specular = directional_light.diffuse * reflectivities[pco.model_index] * dampedSpecular;*/

    vec4 result = ambient + diffuse;
    fragColor = object_colors[pco.model_index] * result;
    fragColor = mix(pco.sky_color, fragColor, visibility);
}