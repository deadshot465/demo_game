#version 450

layout (location = 1) in vec4 inNormal;
layout (location = 2) in vec2 inTexCoord;
layout (location = 3) in vec4 fragPos;

layout (location = 0) out vec4 fragColor;

layout (push_constant) uniform PushConstant
{
    uint texture_index;
    vec4 object_color;
} pco;

layout (binding = 1) uniform DirectionalLight
{
    vec4 diffuse;
    vec3 light_direction;
    float ambient_intensity;
    float specular_intensity;
} direction_light;

//layout (binding = 2) uniform sampler2D TexSampler[80];

void main()
{
    // Texture
    /*vec4 tex_color = texture(TexSampler[pco.texture_index], inTexCoord);
    if (tex_color.a < 0.1) {
        discard;
    }*/

    // Ambient
    //vec4 ambient = direction_light.diffuse * direction_light.ambient_intensity;

    // Diffuse Light
    /*vec4 light_direction = normalize(vec4(-direction_light.light_direction, 0.0));
    vec4 normal = normalize(inNormal);
    float intensity = max(dot(normal, light_direction), 0.0);
    vec4 diffuse = direction_light.diffuse * intensity;

    vec4 result = (ambient + diffuse) * vec4(1.0, 0.0, 0.0, 1.0);*/
    fragColor = pco.object_color;
}