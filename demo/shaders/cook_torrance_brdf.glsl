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

float DistributionGGX2(vec3 N, vec3 H, float roughness)
{
    float NdotH = max(dot(N, H), 0.0);
    float a      = roughness;
    float a2     = a * a;
    float denom  = NdotH * NdotH * (a2 - 1.) + 1.;
    return a2 / (PI * denom * denom);
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

#define SMITH 0
#define epsilon 0.0001

float GGXG1(float NdotV, float a2)
{
    return 2./(1. + sqrt(1. + a2 * (1.-NdotV*NdotV)/(NdotV*NdotV)));
}

float geometrySmith2(float NdotV, float NdotL, float alpha)
{
    float a2 = alpha*alpha;

    #if SMITH == 0
    // uncorrelated Smith
    return GGXG1(NdotL, a2) * GGXG1(NdotV, a2)/(NdotL*NdotV*4.);
    #endif

    #if SMITH == 1
    // height-correlated smith
    float GGXV = NdotL * sqrt(NdotV * NdotV * (1.0 - a2) + a2);
    float GGXL = NdotV * sqrt(NdotL * NdotL * (1.0 - a2) + a2);
    return 0.5 / max(epsilon, (GGXV + GGXL));
    #endif

    #if SMITH == 2
    // height-correlated smith approximation
    return 1./max(epsilon, (2.*mix(2.*NdotL*NdotV, NdotL+NdotV, alpha)));
    #endif
}


// Geometry function using Schlick-GGX approximation
float GeometrySchlickGGX(vec3 N, vec3 V, vec3 L, float roughness) {
    float k = (roughness * roughness) / 2.0;

    float nom = max(dot(N, V), 0);
    float denom = max(dot(N, V), 0) * (1.0 - k) + k;

    return nom / denom;
}

// Fresnel equation using Schlick's approximation
//vec3 FresnelSchlick(float cosTheta, vec3 F0)
//{
//    return F0 + (1.0 - F0) * pow(max(1.0 - cosTheta, 0.0), 5.0);
//}

vec3 FresnelSchlick(float cosTheta, vec3 F0)
{
    return F0 + (1. - F0) * pow(1.0 - cosTheta, 5.);
}

vec3 fresnelSchlickRoughness(float cosTheta, vec3 F0, float roughness)
{
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}


vec3 cook_torrance_brdf(vec3 V, vec3 N, vec3 L, vec3 baseColor, float metallic, float roughness, vec3 light_color) {
    //    return vec3(abs(length(N)-1)*100000);

    vec3 F0 = vec3(0.04);
    F0 = mix(F0, baseColor, metallic);

    vec3 H = normalize(V + L);

    // Calculate DFG terms
    float h_dot_v = dot(H, V);
    vec3 F = FresnelSchlick(h_dot_v, F0);
    //    vec3 F = FresnelSchlick(dot(H, V), F0);

    //    return F;
    //    return vec3(h_dot_v, 0, 0);

    float D = DistributionGGX2(N, H, roughness);

    //    float G = GeometrySchlickGGX(N, V, L, roughness);
    float G = geometrySmith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);
    //    float G = geometrySmith2(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);
    //        float G = geometrySmith(dot(N, V), dot(N, L), roughness);

    //    return vec3(G);

    // Calculate specular and diffuse terms
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diffuse = kD * baseColor * max(dot(N, L), 0.0);

    //    return diffuse;

    //    return vec3(D);
    vec3 specular = (D * G * F);// * envSample;// / (4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.001);
    //    vec3 specular = (D * G * F) / (4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0));
    //    return vec3(D, G, 0);
    //    return specular;

    //    return (1.0 - F) * irradiance * baseColor * (1.0 - metallic);

    // Combine and ensure energy conservation
    return light_color * (diffuse + specular);// Simple approximation to avoid exceeding 1
}

vec3 cook_torrance_brdf_ibl(vec3 V, vec3 N, vec3 baseColor, float metallic, float roughness, sampler2D envmap, sampler2D brdf_lut, vec3 light_color) {
    //    return vec3(abs(length(N)-1)*100000);

    vec3 F0 = mix(vec3(0.04), baseColor, metallic);

    // Calculate DFG terms
    float n_dot_v = max(dot(N, V), 0);
    //    return vec3(n_dot_v, 0, 0);
    //    vec3 F = FresnelSchlick(n_dot_v, F0);
    vec3 F = fresnelSchlickRoughness(n_dot_v, F0, roughness);
    //    vec3 F = FresnelSchlick(dot(H, V), F0);

    //    vec3 R = reflect(-V, N);

    //    return F;
    //    return vec3(h_dot_v, 0, 0);

    //    float D = DistributionGGX2(N, H, roughness);

    //    float G = GeometrySchlickGGX(N, V, L, roughness);
    //    float G = geometrySmith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);
    //    float G = geometrySmith2(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);
    //        float G = geometrySmith(dot(N, V), dot(N, L), roughness);

    //    return vec3(G);

    // Sample environment map and irradiance map
    vec3 irradiance = light_color * sample_environment_map(N, 1.0, envmap).rgb;
    vec3 envSample = light_color* sample_environment_map(reflect(-V, N), roughness, envmap).rgb;

    //    return envSample;

    // Calculate specular and diffuse terms
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    vec3 diffuse = kD * irradiance * baseColor;

    vec3 envBRDF = textureLod(brdf_lut, vec2(n_dot_v, roughness), 0.0).rgb;
    //    envBRDF = pow(envBRDF, vec2(1.0/2.2));

    //    return vec3(roughness);
    //    return vec3(n_dot_v, 0, 0);
    //    return F;

    //    return vec3(1.0)-F;
    //    return vec3(envBRDF);
    vec3 specular = envSample * (F * envBRDF.x + envBRDF.y);
    //    vec3 specular = envSample * (F * envBRDF.x);
    //    return F* envBRDF.x * envSample;

    //    return specular;

    //    return vec3(D);
    //    vec3 specular = (D * G * F);// * envSample;// / (4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.001);
    //    vec3 specular = (D * G * F) / (4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0));
    //    return vec3(D, G, 0);
    //    return specular;

    //    return (1.0 - F) * irradiance * baseColor * (1.0 - metallic);

    // Combine and ensure energy conservation
    return diffuse + specular;// Simple approximation to avoid exceeding 1
}


#endif// COOK_TORRANCE_BRDF_GLSL