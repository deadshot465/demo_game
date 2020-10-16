#version 450

layout (binding = 1) uniform DirectionalLight
{
    vec4 diffuse;
    vec3 light_position;
    float padding0;
    float ambient_intensity;
    float specular_intensity;
} directional_light;

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

layout (location = 1) in vec4 inNormal;
layout (location = 2) in vec2 inTexCoord;
layout (location = 3) in vec4 fragPos;
layout (location = 4) in float visibility;

layout (location = 0) out vec4 fragColor;

void main()
{
    // Texture
    /*vec4 tex_color = texture(tex_sampler, inTexCoord);
    if (tex_color.a < 0.1) {
        discard;
    }*/

    // Ambient
    vec4 ambient = directional_light.diffuse * directional_light.ambient_intensity;

    // Diffuse Light
    vec4 light_direction = vec4(directional_light.light_position, 1.0) - fragPos;
    light_direction = normalize(light_direction);
    vec4 normal = normalize(inNormal);
    float intensity = max(dot(normal, light_direction), 0.0);
    vec4 diffuse = directional_light.diffuse * intensity;

    vec4 result = (ambient + diffuse) * vec4(1.0, 1.0, 1.0, 1.0);
    fragColor = object_colors[pco.model_index] * result;
    fragColor = mix(pco.sky_color, fragColor, visibility);
}