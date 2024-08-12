#ifndef SHADERS_IMAGE_BASED_LIGHTING_GLSL
#define SHADERS_IMAGE_BASED_LIGHTING_GLSL

#include "common.glsl"

vec2 direction_wn_to_spherical_envmap_uv(vec3 direction_wn) {
    // Calculate the azimuthal angle (phi) and the polar angle (theta)
    float phi = atan(direction_wn.z, direction_wn.x);
    float theta = acos(direction_wn.y);

    // Convert angles to UV coordinates
    float u = phi / (2.0 * PI) + 0.5;
    float v = theta / PI;

    return vec2(u, v);
}

vec4 sample_environment_map(vec3 direction_wn, float bias, sampler2D envmap) {
    int levels = textureQueryLevels(envmap);
    float mipLevel = max(float(levels) - 3.5 - (1.0-bias) * 8.0, 0.0);
    vec2 uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureLod(envmap, uv, mipLevel);
}

vec3 sample_srgb_as_linear(sampler2D map, vec2 uv) {
    vec3 v = texture(map, uv).rgb;
    return pow(v, vec3(1.0/2.2));
}

vec3 apply_normal_map_amount(sampler2D normal_map, vec2 uv, vec3 normal, vec3 tangent, float normal_strength) {
    vec3 n = sample_srgb_as_linear(normal_map, uv).rgb;
    n = normalize(n * 2.0 - 1.0);
    mat3 normal_space = mat3(tangent, cross(normal, tangent), normal);
    n = normalize(normal_space * n);
    return normalize(mix(normal, n, normal_strength));
}

#endif// SHADERS_IMAGE_BASED_LIGHTING_GLSL
