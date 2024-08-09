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
    float ambient;
    float normal_strength;
    float normal_map_is_linear;
};

layout (set = 1, binding = 1) uniform sampler2D envmap;
layout (set = 1, binding = 2) uniform sampler2D shadow;

layout (set = 1, binding = 3) uniform sampler2D base_color_map;
layout (set = 1, binding = 4) uniform sampler2D roughness_map;
layout (set = 1, binding = 5) uniform sampler2D metallic_map;
layout (set = 1, binding = 6) uniform sampler2D normal_map;

layout (set = 1, binding = 7) uniform sampler2D brdf_lut;


float adjust(float value, float factor) {
    if (factor < 0.0) {
        return value * (1.0 + factor);
    }
    return factor +value * (1.0 - factor);
}

void main() {
    vec3 mat_color = texture(base_color_map, v_uv).rgb;
    float mat_roughness = sample_srgb_as_linear(roughness_map, v_uv).r;
    float mat_metallic = sample_srgb_as_linear(metallic_map, v_uv).r;

    mat_roughness = adjust(mat_roughness, roughness);
    mat_metallic = adjust(mat_metallic, metallic);

    // normal mapping
    vec3 N = sample_srgb_as_linear(normal_map, v_uv).rgb;
    N = normalize(N * 2.0 - 1.0);
    mat3 TBN = mat3(v_tangent_worldspace, cross(v_normal_worldspace, v_tangent_worldspace), v_normal_worldspace);
    N = normalize(TBN * N);
    N = normalize(mix(v_normal_worldspace, N, normal_strength * 0.5));

    vec3 V = normalize(v_camera_pos_worldspace - v_pos_worldspace);
    vec3 L = g_light_dir_worldspace_norm;

    vec3 color_acc = vec3(0);
    color_acc += cook_torrance_brdf(V, N, L, mat_color.rgb, mat_metallic, mat_roughness, color.rgb);
    color_acc += cook_torrance_brdf_ibl(V, N, mat_color.rgb, mat_metallic, mat_roughness, envmap, brdf_lut, vec3(ambient));

    // hdr
    color_acc = color_acc / (color_acc + vec3(1.0));

    f_color = vec4(color_acc, 1.0);
}
