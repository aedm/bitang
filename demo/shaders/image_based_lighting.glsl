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
    vec2 uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureLod(envmap, uv, bias);
}

#endif // SHADERS_IMAGE_BASED_LIGHTING_GLSL
