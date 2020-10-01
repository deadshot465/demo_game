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

layout (set = 1, binding = 0) uniform sampler2D tex_sampler;
layout (vertices = 4) out;
layout (location = 0) in vec4 inNormal[];
layout (location = 1) in vec2 inUV[];
layout (location = 0) out vec4 outNormal[4];
layout (location = 1) out vec2 outUV[4];

layout (push_constant) uniform TessPushConstant {
    vec2 viewport_dim;
    float tessellation_edge_size;
    float tessellation_factor;
} pco;

// Calculate the tessellation factor based on screen space
// dimensions of the edge
float screenSpaceTessFactor(vec4 p0, vec4 p1) {
    // Calculate edge mid-point
    vec4 mid_point = 0.5 * (p0 + p1);
    // Sphere radius as distance between the control points
    float radius = distance(p0, p1) / 2.0;

    // View space
    vec4 v0 = mvp.view * dbo.model * mid_point;
    // Project into clip space
    vec4 clip0 = (mvp.projection * (v0 - vec4(radius, vec3(0.0))));
    vec4 clip1 = (mvp.projection * (v0 + vec4(radius, vec3(0.0))));

    // Get normalized device coordinates
    clip0 /= clip0.w;
    clip1 /= clip1.w;
    // Convert to viewport coordinates
    clip0.xy *= pco.viewport_dim;
    clip1.xy *= pco.viewport_dim;
    // Return the tessellation factor based on the screen size
    // given by the distance of the two edge control points in screen space
    // and a reference (min.) tessellation size for the edge set by the application
    return clamp(distance(clip0, clip1) / pco.tessellation_edge_size * pco.tessellation_factor, 1.0, 64.0);
}

void main() {
    if (gl_InvocationID == 0) {
        if (pco.tessellation_factor > 0.0) {

        }
    }
}
