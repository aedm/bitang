#version 450

#include "/shaders/cook_torrance_brdf.glsl"

layout (location = 0) in vec2 v_uv;
layout (location = 1) in vec3 v_normal_worldspace;
layout (location = 2) in vec3 v_tangent_worldspace;
layout (location = 3) in vec3 v_pos_worldspace;
layout (location = 4) in vec3 v_camera_pos_worldspace;

layout (location = 5) in vec3 v_material_adjustment;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Uniforms {
    mat4 g_light_projection_from_world;
    mat4 g_camera_from_world;
    mat4 g_projection_from_camera;
    float g_chart_time;
    float g_app_time;
    vec3 g_light_dir_worldspace_norm;

    vec4 color;
    float roughness;
    float metallic;
    float ambient;
    float normal_strength;
    float shadow_bias;
    float clearcoat;
} u;

layout (set = 1, binding = 1) uniform sampler2D envmap;
layout (set = 1, binding = 2) uniform sampler2DShadow shadow;

layout (set = 1, binding = 3) uniform sampler2D base_color_map;
layout (set = 1, binding = 4) uniform sampler2D roughness_map;
layout (set = 1, binding = 5) uniform sampler2D metallic_map;
layout (set = 1, binding = 6) uniform sampler2D normal_map;

layout (set = 1, binding = 7) uniform sampler2D brdf_lut;
layout (set = 1, binding = 8) uniform sampler2D lightmap;


float adjust(float value, float factor) {
    if (factor < 0.0) {
        return value * (1.0 + factor);
    }
    return factor + value * (1.0 - factor);
}

float sample_shadow_map(vec3 world_pos) {
    vec3 lightspace_pos = (u.g_light_projection_from_world * vec4(world_pos, 1.0)).xyz;
    lightspace_pos.xy = lightspace_pos.xy * 0.5 + 0.5;
    lightspace_pos.z -= u.shadow_bias * 0.001;
    return texture(shadow, lightspace_pos);
}

void main() {
    vec2 uv = v_uv;
    vec3 base_color = texture(base_color_map, uv).rgb;

    float roughness = sample_srgb_as_linear(roughness_map, uv).r;
    float metallic = sample_srgb_as_linear(metallic_map, uv).r;

    float light = sample_shadow_map(v_pos_worldspace);

//    roughness = adjust(roughness, v_material_adjustment.x * 2.0 - 1.0);
//    metallic = adjust(metallic, v_material_adjustment.y * 2.0 - 1.0);

    roughness = adjust(roughness, u.roughness);
    metallic = adjust(metallic, u.metallic);

    float clearcoat = adjust(roughness, u.clearcoat);


    vec3 normal_wn = normalize(v_normal_worldspace);
    vec3 tangent_wn = normalize(v_tangent_worldspace);

    vec3 N = apply_normal_map_amount(normal_map, uv, normal_wn, tangent_wn, u.normal_strength);
    vec3 V = normalize(v_camera_pos_worldspace - v_pos_worldspace);
    vec3 L = u.g_light_dir_worldspace_norm;

    vec3 light_y = -u.g_light_dir_worldspace_norm;
    vec3 light_x = normalize(cross(light_y, vec3(0, 0, 1)));
    vec3 light_z = cross(light_x, light_y);
//    mat3 light_transform = mat3(vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1));
//    vec3 light_z = u.g_light_dir_worldspace_norm;
    mat3 light_transform = mat3(-light_y, -light_x, light_z);


    vec3 color_acc = vec3(0);
//    color_acc += cook_torrance_brdf_ibl(V, N, base_color.rgb, metallic, roughness, envmap, brdf_lut, vec3(u.ambient));
//    color_acc += cook_torrance_brdf(V, N, L, base_color.rgb, metallic, roughness, u.color.rgb * light);
    color_acc += cook_torrance_brdf_lightmap(V, N, L, base_color.rgb, metallic, clearcoat, lightmap, brdf_lut, u.color.rgb * light, light_transform);

    f_color = vec4(color_acc, 1.0);
}
