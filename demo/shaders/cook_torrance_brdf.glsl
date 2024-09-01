#ifndef COOK_TORRANCE_BRDF_GLSL
#define COOK_TORRANCE_BRDF_GLSL

#include "image_based_lighting.glsl"
#include "common.glsl"

// Fresnel-Schlick approximation
vec3 fresnel_schlick(float cosTheta, vec3 F0) {
    return F0 + (1. - F0) * pow(1.0 - cosTheta, 5.);
}

// Fresnel-Schlick approximation with roughness compensation, looks a bit better
vec3 fresnel_schlick_roughness(float cosTheta, vec3 F0, float roughness) {
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// GGX/Trowbridge-Reitz normal distribution function
float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float NdotH = max(dot(N, H), 0.0);
    float a = roughness * roughness;
    float denom  = NdotH * NdotH * (a - 1.0) + 1.0;
    return a / (PI * denom * denom);
}

// Geometry function (Smith's method with GGX)
float geometry_smith(float NdotV, float NdotL, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    float ggx1 = NdotV / (NdotV * (1.0 - k) + k);
    float ggx2 = NdotL / (NdotL * (1.0 - k) + k);
    return ggx1 * ggx2;
}

// Cook-torrance BRDF using a single lambertian point light source
vec3 cook_torrance_brdf(vec3 V, vec3 N, vec3 L, vec3 baseColor, float metallic, float roughness, vec3 light_color) {
    vec3 F0 = vec3(0.04);
    F0 = mix(F0, baseColor, metallic);

    vec3 H = normalize(V + L);

    // Calculate DFG terms
    float h_dot_v = dot(H, V);
    vec3 F = fresnel_schlick(h_dot_v, F0);
    float D = distribution_ggx(N, H, roughness);
    float G = geometry_smith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);

    // Calculate specular and diffuse terms
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diffuse = kD * baseColor * max(dot(N, L), 0.0);
    vec3 specular = (D * G * F);

    // Combine and ensure energy conservation
    return light_color * (diffuse + specular);
}

// Cook-torrance BRDF using image based lighting
vec3 cook_torrance_brdf_ibl(vec3 V, vec3 N, vec3 baseColor, float metallic, float roughness, sampler2D envmap, sampler2D brdf_lut, vec3 light_color) {
    vec3 F0 = mix(vec3(0.04), baseColor, metallic);

    // Calculate DFG terms
    float n_dot_v = max(dot(N, V), 0);
    vec3 F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

    // Sample environment map and irradiance map
    vec3 irradiance = light_color * sample_environment_map(N, 1.0, envmap).rgb;
    vec3 envSample = light_color * sample_environment_map(reflect(-V, N), roughness, envmap).rgb;

    // Calculate specular and diffuse terms
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diffuse = kD * irradiance * baseColor;

    vec3 envBRDF = textureLod(brdf_lut, vec2(n_dot_v, roughness), 0.0).rgb;
    vec3 specular = envSample * (F * envBRDF.x + envBRDF.y);

    // Combine and ensure energy conservation
    return diffuse + specular;// Simple approximation to avoid exceeding 1
}

vec3 cook_torrance_brdf_lightmap(vec3 V, vec3 N, vec3 L, vec3 baseColor, float metallic, float roughness, sampler2D envmap, sampler2D brdf_lut, vec3 light_color) {
    mat3 light_from_world = make_lightspace_from_worldspace_transformation(L);
    vec3 F0 = mix(vec3(0.04), baseColor, metallic);

    // Calculate DFG terms
    float n_dot_v = max(dot(N, V), 0);
    vec3 F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

    // Sample environment map and irradiance map
    vec3 normal_lightspace = light_from_world * N;
    vec3 irradiance = light_color * sample_environment_map(normal_lightspace, 1.0, envmap).rgb;
    vec3 reflection = reflect(-V, N);
    vec3 reflection_lightspace = light_from_world * reflection;
    vec3 envSample = light_color * sample_environment_map(reflection_lightspace, roughness, envmap).rgb;

    // Calculate specular and diffuse terms
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diffuse = kD * irradiance * baseColor * max(dot(N, L), 0.0);

    vec3 envBRDF = textureLod(brdf_lut, vec2(n_dot_v, roughness), 0.0).rgb;
    vec3 specular = envSample * (F * envBRDF.x + envBRDF.y) * max(dot(L, N), 0.0);

    // Combine and ensure energy conservation
    return diffuse + specular;// Simple approximation to avoid exceeding 1
}

#endif// COOK_TORRANCE_BRDF_GLSL