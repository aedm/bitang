#version 450

#include "/shaders/cook_torrance_brdf.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 1) in vec3 v_normal_worldspace;
layout (location = 2) in vec3 v_tangent_worldspace;
layout (location = 3) in vec3 v_pos_worldspace;
layout (location = 4) in vec3 v_camera_pos_worldspace;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    mat4 g_light_projection_from_world;
    mat4 g_camera_from_world;
    mat4 g_projection_from_camera;
    float g_chart_time;
    float g_app_time;
    vec3 g_light_dir_camspace_norm;
    vec3 g_light_dir_worldspace_norm;

    float roughness;
    float metallic;
    vec4 color;
};

layout (set = 1, binding = 1) uniform sampler2D envmap;
layout (set = 1, binding = 2) uniform sampler2D shadow;


void main() {
    vec3 V = normalize(v_camera_pos_worldspace - v_pos_worldspace);
    vec3 N = normalize(v_normal_worldspace);
    vec3 L = g_light_dir_worldspace_norm;
    vec3 color = CookTorranceBRDF(V, N, L, color.rgb, metallic, roughness, envmap);

    //    f_color = vec4(v_world_normal, 1);
    //    return;

    //    vec3 base_color = color.rgb;
    //
    //    // Calculate final color
    //    vec3 color = light_pixel(v_world_pos, v_world_normal, 1.0, ambient, base_color, 0.0);
    f_color = vec4(color, 1.0);
}
