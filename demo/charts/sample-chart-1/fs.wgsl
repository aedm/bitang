struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
    @location(1) v_normal_worldspace: vec3<f32>,
    @location(2) v_tangent_worldspace: vec3<f32>,
    @location(3) v_pos_worldspace: vec3<f32>,
    @location(4) v_camera_pos_worldspace: vec3<f32>,
    @location(5) v_material_adjustment: vec3<f32>,
};

struct Uniforms {
    g_light_projection_from_world: mat4x4<f32>,
    g_camera_from_world: mat4x4<f32>,
    g_projection_from_camera: mat4x4<f32>,
    g_chart_time: f32,
    g_app_time: f32,
    g_light_dir_camspace_norm: vec3<f32>,
    g_light_dir_worldspace_norm: vec3<f32>,
    light_color: vec4<f32>,
    roughness: f32,
    metallic: f32,
    ambient: f32,
    normal_strength: f32,
    shadow_bias: f32,
    color: vec3<f32>,
};

@group(1) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(1) var envmap: texture_2d<f32>;
@group(1) @binding(2) var shadow: texture_depth_2d;
@group(1) @binding(3) var base_color_map: texture_2d<f32>;
@group(1) @binding(4) var roughness_map: texture_2d<f32>;
@group(1) @binding(5) var metallic_map: texture_2d<f32>;
@group(1) @binding(6) var normal_map: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut: texture_2d<f32>;

@group(1) @binding(11) var sampler_envmap: sampler;
@group(1) @binding(12) var sampler_shadow: sampler_comparison;
@group(1) @binding(13) var sampler_repeat: sampler;

const PI: f32 = 3.14159265359;

fn direction_wn_to_spherical_envmap_uv(direction_wn: vec3<f32>) -> vec2<f32> {
    // Calculate the azimuthal angle (phi) and the polar angle (theta)
    let phi = atan2(direction_wn.z, direction_wn.x);
    let theta = acos(direction_wn.y);

    // Convert angles to UV coordinates
    let u = phi / (2.0 * PI) + 0.25;
    let v = theta / PI;

    return vec2<f32>(u, v);
}

fn sample_environment_map(direction_wn: vec3<f32>, bias: f32, envmap: texture_2d<f32>) -> vec4<f32> {
    let levels = textureNumLevels(envmap);
    let adjust = pow(1.0 - bias, 4.0);
    let mipLevel = max(f32(levels) - 3.5 - adjust * 7.0, 0.0);
    let uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureSampleLevel(envmap, sampler_envmap, uv, mipLevel);
}

fn sample_srgb_as_linear(map: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
    let v = textureSample(map, sampler_repeat, uv).rgb;
    return pow(v, vec3<f32>(1.0/2.2));
}

fn apply_normal_map_amount(normal_map: texture_2d<f32>, uv: vec2<f32>, normal_n: vec3<f32>, tangent_n: vec3<f32>, normal_strength: f32) -> vec3<f32> {
    let normal_space = mat3x3<f32>(
        tangent_n,
        cross(normal_n, tangent_n),
        normal_n
    );
    var n = sample_srgb_as_linear(normal_map, uv);
    n = normalize(n * 2.0 - 1.0);
    n = normal_space * n;
    return normalize(mix(normal_n, n, normal_strength));
}

fn make_lightspace_from_worldspace_transformation(light_dir_worldspace_n: vec3<f32>) -> mat3x3<f32> {
    // Axes of lightspace expressed in worldspace
    let light_z = light_dir_worldspace_n;
    let light_x = normalize(cross(light_z, vec3<f32>(0.0, 1.0, 0.0)));
    let light_y = cross(light_x, light_z);

    // Orthonormal transformation from lightspace to worldspace
    let world_from_light = mat3x3<f32>(
        light_x,
        light_y,
        light_z
    );

    // We need the inverse to transform worldspace vectors to lightspace.
    // For orthonormal vectors, transpose is the same as inverse but cheaper.
    let light_from_world = transpose(world_from_light);

    return light_from_world;
}

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

fn cook_torrance_brdf_ibl(V: vec3<f32>, N: vec3<f32>, baseColor: vec3<f32>, metallic: f32, roughness: f32, envmap: texture_2d<f32>, brdf_lut: texture_2d<f32>, light_color: vec3<f32>) -> vec3<f32> {
    let F0 = mix(vec3<f32>(0.04), baseColor, metallic);

    // Calculate DFG terms
    let n_dot_v = max(dot(N, V), 0.0);
    let F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

    // Sample environment map and irradiance map
    let irradiance = light_color * sample_environment_map(N, 1.0, envmap).rgb;
    let envSample = light_color * sample_environment_map(reflect(-V, N), roughness, envmap).rgb;

    // Calculate specular and diffuse terms
    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * irradiance * baseColor;

    let envBRDF = textureSampleLevel(brdf_lut, sampler_envmap, vec2<f32>(n_dot_v, roughness), 0.0).rgb;
    let specular = envSample * (F * envBRDF.x + envBRDF.y);

    // Combine and ensure energy conservation
    return diffuse + specular;
}

// Note: The sample_environment_map function needs to be implemented separately
// as it depends on your specific environment mapping implementation
fn cook_torrance_brdf_lightmap(V: vec3<f32>, N: vec3<f32>, L: vec3<f32>,
    baseColor: vec3<f32>, metallic: f32, roughness: f32, envmap: texture_2d<f32>,
    brdf_lut: texture_2d<f32>, light_color: vec3<f32>) -> vec3<f32> {
    let light_from_world = make_lightspace_from_worldspace_transformation(L);
    let F0 = mix(vec3<f32>(0.04), baseColor, metallic);

    // Calculate DFG terms
    let n_dot_v = max(dot(N, V), 0.0);
    let F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

    // Sample environment map and irradiance map
    let normal_lightspace = light_from_world * N;
    let irradiance = light_color * sample_environment_map(normal_lightspace, 1.0, envmap).rgb;
    let reflection = reflect(-V, N);
    let reflection_lightspace = light_from_world * reflection;
    let envSample = light_color * sample_environment_map(reflection_lightspace, roughness, envmap).rgb;

    // Calculate specular and diffuse terms
    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * irradiance * baseColor * max(dot(N, L), 0.0);

    let envBRDF = textureSampleLevel(brdf_lut, sampler_envmap, vec2<f32>(n_dot_v, roughness), 0.0).rgb;
    let specular = envSample * (F * envBRDF.x + envBRDF.y) * max(dot(L, N), 0.0);

    // Combine and ensure energy conservation
    return diffuse + specular;
}


fn adjust(value: f32, factor: f32) -> f32 {
    if (factor < 0.0) {
        return value * (1.0 + factor);
    }
    return factor + value * (1.0 - factor);
}

fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {
    var lightspace_pos = (u.g_light_projection_from_world * vec4<f32>(world_pos, 1.0)).xyz;
    lightspace_pos = lightspace_pos * vec3f(0.5, -0.5, 1) + vec3f(0.5, 0.5, u.shadow_bias * -0.001);
    return textureSampleCompare(shadow, sampler_shadow, lightspace_pos.xy, lightspace_pos.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.v_uv * 1.0;
    var base_color = textureSample(base_color_map, sampler_repeat, uv).rgb;
    var roughness = textureSample(roughness_map, sampler_repeat, uv).r;
    var metallic = textureSample(metallic_map, sampler_repeat, uv).r;

    let light = sample_shadow_map(in.v_pos_worldspace);

    let normal_wn = normalize(in.v_normal_worldspace);
    let tangent_wn = normalize(in.v_tangent_worldspace);

    let N = apply_normal_map_amount(normal_map, uv, normal_wn, tangent_wn, u.normal_strength);
    let V = normalize(in.v_camera_pos_worldspace - in.v_pos_worldspace);
    let L = u.g_light_dir_worldspace_norm;

    var color_acc = vec3<f32>(0.0);
    color_acc += cook_torrance_brdf(V, N, L, base_color, metallic, roughness, u.light_color.rgb * light);
    color_acc += cook_torrance_brdf_ibl(V, N, base_color, metallic, roughness, envmap, brdf_lut, vec3f(u.ambient));

    return vec4<f32>(color_acc, 1.0);
}
