#version 450
#extension GL_EXT_nonuniform_qualifier : require

layout (binding = 1) uniform DirectionalLight
{
    vec4 diffuse;
    vec3 light_direction;
    float padding0;
    float ambient_intensity;
    float specular_intensity;
} direction_light;

layout (binding = 3) uniform sampler2D tex_sampler[];

layout (push_constant) uniform PushConstant
{
    uint texture_index;
    vec4 object_color;
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
    vec4 tex_color = texture(tex_sampler[pco.texture_index], inTexCoord);
    if (tex_color.a < 0.1) {
        discard;
    }

    // Ambient
    vec4 ambient = direction_light.diffuse * direction_light.ambient_intensity;

    // Diffuse Light
    vec4 light_direction = normalize(vec4(-direction_light.light_direction, 0.0));
    vec4 normal = normalize(inNormal);
    float intensity = max(dot(normal, light_direction), 0.0);
    vec4 diffuse = direction_light.diffuse * intensity;

    //vec4 result = (ambient + diffuse) * vec4(1.0, 0.0, 0.0, 1.0);
    fragColor = vec4(1.0, 1.0, 1.0, 1.0) * tex_color;
    fragColor = mix(pco.sky_color, fragColor, visibility);
}