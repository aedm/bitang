// Constants
const PI: f32 = 3.141592653589793;

// Fresnel-Schlick approximation
fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (vec3<f32>(1.0) - F0) * pow(1.0 - cosTheta, 5.0);
}

// Fresnel-Schlick approximation with roughness compensation
fn fresnel_schlick_roughness(cosTheta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3<f32>(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// GGX/Trowbridge-Reitz normal distribution function
fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let NdotH = max(dot(N, H), 0.0);
    let a = roughness * roughness;
    let denom = NdotH * NdotH * (a - 1.0) + 1.0;
    return a / (PI * denom * denom);
}

// Geometry function (Smith's method with GGX)
fn geometry_smith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    let ggx1 = NdotV / (NdotV * (1.0 - k) + k);
    let ggx2 = NdotL / (NdotL * (1.0 - k) + k);
    return ggx1 * ggx2;
}

// Cook-torrance BRDF using a single lambertian point light source
fn cook_torrance_brdf(V: vec3<f32>, N: vec3<f32>, L: vec3<f32>, baseColor: vec3<f32>, 
    metallic: f32, roughness: f32, light_color: vec3<f32>) -> vec3<f32> {
    var F0 = vec3<f32>(0.04);
    F0 = mix(F0, baseColor, metallic);

    let H = normalize(V + L);

    // Calculate DFG terms
    let h_dot_v = dot(H, V);
    let F = fresnel_schlick(h_dot_v, F0);
    let D = distribution_ggx(N, H, roughness);
    let G = geometry_smith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);

    // Calculate specular and diffuse terms
    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * baseColor * max(dot(N, L), 0.0);
    let specular = (D * G * F);

    // Combine and ensure energy conservation
    return light_color * (diffuse + specular);
}

// Cook-torrance BRDF using image based lighting
@group(0) @binding(0) var env_sampler: sampler;
@group(0) @binding(1) var envmap: texture_2d<f32>;
@group(0) @binding(2) var brdf_lut: texture_2d<f32>;

fn cook_torrance_brdf_ibl(V: vec3<f32>, N: vec3<f32>, baseColor: vec3<f32>, 
    metallic: f32, roughness: f32, light_color: vec3<f32>) -> vec3<f32> {
    let F0 = mix(vec3<f32>(0.04), baseColor, metallic);

    // Calculate DFG terms
    let n_dot_v = max(dot(N, V), 0.0);
    let F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

    // Sample environment map and irradiance map
    let irradiance = light_color * sample_environment_map(N, 1.0);
    let envSample = light_color * sample_environment_map(reflect(-V, N), roughness);

    // Calculate specular and diffuse terms
    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * irradiance * baseColor;

    let envBRDF = textureSampleLevel(brdf_lut, env_sampler, vec2<f32>(n_dot_v, roughness), 0.0).rgb;
    let specular = envSample * (F * envBRDF.x + envBRDF.y);

    // Combine and ensure energy conservation
    return diffuse + specular;
}

// Note: The sample_environment_map function needs to be implemented separately
// as it depends on your specific environment mapping implementation
