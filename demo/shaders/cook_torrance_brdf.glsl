#ifndef COOK_TORRANCE_BRDF_GLSL
#define COOK_TORRANCE_BRDF_GLSL

#include "image_based_lighting.glsl"
#include "common.glsl"


// GGX/Trowbridge-Reitz normal distribution function
float DistributionGGX(vec3 N, vec3 H, float roughness) {
    float a = roughness*roughness;
    float a2 = a*a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH*NdotH;

    float nom   = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = 3.14159 * denom * denom;

    return nom / denom;
}

// Function to calculate the Fresnel term using Schlick's approximation
//vec3 fresnelSchlick(float cosTheta, vec3 F0) {
//    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
//}


// Geometry function (Smith's method with GGX)
float geometrySmith(float NdotV, float NdotL, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    float ggx1 = NdotV / (NdotV * (1.0 - k) + k);
    float ggx2 = NdotL / (NdotL * (1.0 - k) + k);
    return ggx1 * ggx2;
}


// Geometry function using Schlick-GGX approximation
float GeometrySchlickGGX(vec3 N, vec3 V, vec3 L, float roughness) {
    float k = (roughness * roughness) / 2.0;

    float nom = dot(N, V);
    float denom = dot(N, V) * (1.0 - k) + k;

    return nom / denom;
}

// Fresnel equation using Schlick's approximation
vec3 FresnelSchlick(float cosTheta, vec3 F0)
{
    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
}


// Cook-Torrance BRDF using Schlick and GGX
vec3 CookTorranceBRDF(vec3 V, vec3 N, vec3 L, vec3 baseColor, float metallic, float roughness, sampler2D envmap) {
    vec3 F0 = vec3(0.04);
    F0 = mix(F0, baseColor, metallic);

    vec3 H = normalize(V + L);

    // Calculate DFG terms
    float D = DistributionGGX(N, H, roughness);
    vec3 F = FresnelSchlick(max(dot(H, V), 0.0), F0);
    //    float G = GeometrySchlickGGX(N, V, L, roughness);
    float G = geometrySmith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);

    // Sample environment map and irradiance map
    //    vec3 envSample = textureLod(environmentMap, reflect(-V, N), roughness * 8.0).rgb;// Sample with roughness-based mip level
    //    vec3 irradiance = textureLod(irradianceMap, N, 0).rgb;// Sample highest mip level
    vec3 irradiance = sample_environment_map(N, 5.5, envmap).rgb;
    vec3 envSample = sample_environment_map(reflect(-V, N), roughness * 8.0, envmap).rgb;

    // Calculate specular and diffuse terms
    vec3 specular = (D * G * F) * envSample / (4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.001);
    vec3 diffuse = (1.0 - F) * irradiance * baseColor * (1.0 - metallic);

    //    return (1.0 - F) * irradiance * baseColor * (1.0 - metallic);
    //    return specular;
    //    return irradiance;
    //    return envSample;

    // Combine and ensure energy conservation
    return diffuse + specular * (1.0 / (1.0 + D));// Simple approximation to avoid exceeding 1
}


#endif// COOK_TORRANCE_BRDF_GLSL