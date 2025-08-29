import package::shaders::pbr::pbr_material;
// const PI: f32 = 3.14159265359;

struct VertexInput {
    @location(0) a_position: vec3<f32>,
    @location(1) a_normal: vec3<f32>,
    @location(2) a_tangent: vec3<f32>,
    @location(3) a_uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) v_uv: vec2<f32>,
    @location(1) v_normal_worldspace: vec3<f32>,
    @location(2) v_tangent_worldspace: vec3<f32>,
    @location(3) v_pos_worldspace: vec3<f32>,
    @location(4) v_camera_pos_worldspace: vec3<f32>,
    @location(5) v_material_adjustment: vec3<f32>,
};

// Vertex shader inputs
struct VsUniforms {
    g_projection_from_world: mat4x4<f32>,
    g_projection_from_model: mat4x4<f32>,
    g_camera_from_model: mat4x4<f32>,
    g_camera_from_world: mat4x4<f32>,
    g_world_from_model: mat4x4<f32>,
    g_light_dir_worldspace_norm: vec3<f32>,
    g_app_time: f32,
    g_simulation_frame_ratio: f32,
    instance_move: vec3<f32>,
};

struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
    upvector: vec3<f32>,
}

@group(0) @binding(0) var<uniform> context: VsUniforms;
@group(0) @binding(1) var<storage, read> particles_current: array<Particle>;
@group(0) @binding(2) var<storage, read_write> particles_next: array<Particle>;


struct FsUniforms {
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

// Fragment shader inputs
@group(1) @binding(0) var<uniform> u: FsUniforms;
@group(1) @binding(1) var envmap: texture_2d<f32>;
@if(!ENTRY_POINT_FS_MAIN_NOOP) 
@group(1) @binding(2) var shadow: texture_depth_2d;
@group(1) @binding(3) var base_color_map: texture_2d<f32>;
@group(1) @binding(4) var roughness_map: texture_2d<f32>;
@group(1) @binding(5) var metallic_map: texture_2d<f32>;
@group(1) @binding(6) var normal_map: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut: texture_2d<f32>;

@group(1) @binding(11) var sampler_envmap: sampler;
@group(1) @binding(12) var sampler_shadow: sampler_comparison;
@group(1) @binding(13) var sampler_repeat: sampler;

// fn direction_wn_to_spherical_envmap_uv(direction_wn: vec3<f32>) -> vec2<f32> {
//     // Calculate the azimuthal angle (phi) and the polar angle (theta)
//     let phi = atan2(direction_wn.z, direction_wn.x);
//     let theta = acos(direction_wn.y);

//     // Convert angles to UV coordinates
//     let u = phi / (2.0 * PI) + 0.25;
//     let v = theta / PI;

//     return vec2<f32>(u, v);
// }

// fn sample_environment_map(direction_wn: vec3<f32>, bias: f32, 
//     envmap: texture_2d<f32>, sampler_envmap: sampler) -> vec4<f32> {
//     let levels = textureNumLevels(envmap);
//     let adjust = pow(1.0 - bias, 4.0);
//     let mipLevel = max(f32(levels) - 3.5 - adjust * 7.0, 0.0);

//     let uv = direction_wn_to_spherical_envmap_uv(direction_wn);
//     return textureSampleLevel(envmap, sampler_envmap, uv, mipLevel);
// }

// fn sample_srgb_as_linear(map: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
//     let v = textureSample(map, sampler_repeat, uv).rgb;
//     return pow(v, vec3<f32>(1.0 / 2.2));
// }

// fn apply_normal_map_amount(normal_map: texture_2d<f32>, uv: vec2<f32>, 
//     normal_n: vec3<f32>, tangent_n: vec3<f32>, normal_strength: f32) -> vec3<f32> {
//     let normal_space = mat3x3<f32>(
//         tangent_n,
//         cross(normal_n, tangent_n),
//         normal_n
//     );
//     var n = sample_srgb_as_linear(normal_map, uv);
//     n = normalize(n * 2.0 - 1.0);
//     n = normal_space * n;
//     return normalize(mix(normal_n, n, normal_strength));
// }

// fn make_lightspace_from_worldspace_transformation(light_dir_worldspace_n: vec3<f32>) -> mat3x3<f32> {
//     // Axes of lightspace expressed in worldspace
//     let light_z = light_dir_worldspace_n;
//     let light_x = normalize(cross(light_z, vec3<f32>(0.0, 1.0, 0.0)));
//     let light_y = cross(light_x, light_z);

//     // Orthonormal transformation from lightspace to worldspace
//     let world_from_light = mat3x3<f32>(
//         light_x,
//         light_y,
//         light_z
//     );

//     // We need the inverse to transform worldspace vectors to lightspace.
//     // For orthonormal vectors, transpose is the same as inverse but cheaper.
//     let light_from_world = transpose(world_from_light);

//     return light_from_world;
// }

// // Fresnel-Schlick approximation
// fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
//     return F0 + (vec3<f32>(1.0) - F0) * pow(1.0 - cosTheta, 5.0);
// }

// // Fresnel-Schlick approximation with roughness compensation
// fn fresnel_schlick_roughness(cosTheta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
//     return F0 + (max(vec3<f32>(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
// }

// // GGX/Trowbridge-Reitz normal distribution function
// fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
//     let NdotH = max(dot(N, H), 0.0);
//     let a = roughness * roughness;
//     let denom = NdotH * NdotH * (a - 1.0) + 1.0;
//     return a / (PI * denom * denom);
// }

// // Geometry function (Smith's method with GGX)
// fn geometry_smith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
//     let r = roughness + 1.0;
//     let k = (r * r) / 8.0;
//     let ggx1 = NdotV / (NdotV * (1.0 - k) + k);
//     let ggx2 = NdotL / (NdotL * (1.0 - k) + k);
//     return ggx1 * ggx2;
// }

// // Cook-torrance BRDF using a single lambertian point light source
// fn cook_torrance_brdf(V: vec3<f32>, N: vec3<f32>, L: vec3<f32>, baseColor: vec3<f32>,
//     metallic: f32, roughness: f32, light_color: vec3<f32>) -> vec3<f32> {
//     var F0 = vec3<f32>(0.04);
//     F0 = mix(F0, baseColor, metallic);

//     let H = normalize(V + L);

//     // Calculate DFG terms
//     let h_dot_v = dot(H, V);
//     let F = fresnel_schlick(h_dot_v, F0);
//     let D = distribution_ggx(N, H, roughness);
//     let G = geometry_smith(max(dot(N, V), 0.0), max(dot(N, L), 0.0), roughness);

//     // Calculate specular and diffuse terms
//     let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
//     let diffuse = kD * baseColor * max(dot(N, L), 0.0);
//     let specular = (D * G * F);

//     // Combine and ensure energy conservation
//     return light_color * (diffuse + specular);
// }

// fn cook_torrance_brdf_ibl(V: vec3<f32>, N: vec3<f32>, baseColor: vec3<f32>, 
//     metallic: f32, roughness: f32, envmap: texture_2d<f32>, brdf_lut: texture_2d<f32>, 
//     light_color: vec3<f32>, sampler_envmap: sampler) -> vec3<f32> {
//     let F0 = mix(vec3<f32>(0.04), baseColor, metallic);

//     // Calculate DFG terms
//     let n_dot_v = max(dot(N, V), 0.0);
//     let F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

//     // Sample environment map and irradiance map
//     var irradiance = light_color * sample_environment_map(N, 1.0, envmap, sampler_envmap).rgb;
//     let envSample = light_color * 
//         sample_environment_map(reflect(-V, N), roughness, envmap, sampler_envmap).rgb;

//     // Calculate specular and diffuse terms
//     let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
//     let diffuse = kD * irradiance * baseColor;

//     var envBRDF = textureSampleLevel(brdf_lut, sampler_envmap, vec2<f32>(n_dot_v, roughness), 0.0).rgb;
//     let specular = envSample * (F * envBRDF.x + envBRDF.y);

//     // Combine and ensure energy conservation
//     return diffuse + specular;
// }

// // Note: The sample_environment_map function needs to be implemented separately
// // as it depends on your specific environment mapping implementation
// fn cook_torrance_brdf_lightmap(V: vec3<f32>, N: vec3<f32>, L: vec3<f32>,
//     baseColor: vec3<f32>, metallic: f32, roughness: f32, envmap: texture_2d<f32>,
//     brdf_lut: texture_2d<f32>, light_color: vec3<f32>, sampler_envmap: sampler) -> vec3<f32> {
//     let light_from_world = make_lightspace_from_worldspace_transformation(L);
//     let F0 = mix(vec3<f32>(0.04), baseColor, metallic);

//     // Calculate DFG terms
//     let n_dot_v = max(dot(N, V), 0.0);
//     let F = fresnel_schlick_roughness(n_dot_v, F0, roughness);

//     // Sample environment map and irradiance map
//     let normal_lightspace = light_from_world * N;
//     let irradiance = light_color * sample_environment_map(normal_lightspace, 1.0, 
//         envmap, sampler_envmap).rgb;
//     let reflection = reflect(-V, N);
//     let reflection_lightspace = light_from_world * reflection;
//     let envSample = light_color * sample_environment_map(reflection_lightspace, roughness, 
//         envmap, sampler_envmap).rgb;

//     // Calculate specular and diffuse terms
//     let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
//     let diffuse = kD * irradiance * baseColor * max(dot(N, L), 0.0);

//     let envBRDF = textureSampleLevel(brdf_lut, sampler_envmap, vec2<f32>(n_dot_v, roughness), 0.0).rgb;
//     let specular = envSample * (F * envBRDF.x + envBRDF.y) * max(dot(L, N), 0.0);

//     // Combine and ensure energy conservation
//     return diffuse + specular;
// }


// fn adjust(value: f32, factor: f32) -> f32 {
//     if factor < 0.0 {
//         return value * (1.0 + factor);
//     }
//     return factor + value * (1.0 - factor);
// }

// fn pbr_material(uv: vec2f, pos_w: vec3f, normal_w: vec3f, tangent_w: vec3f, 
//     camera_pos_w: vec3f, light_dir_wn: vec3f,
//     normal_strength: f32, light_color: vec3f, ambient_color: vec3f, 
//     roughness_adjust: f32, metallic_adjust: f32,
//     base_color_map: texture_2d<f32>, roughness_map: texture_2d<f32>, 
//     metallic_map: texture_2d<f32>, normal_map: texture_2d<f32>,
//     envmap: texture_2d<f32>, brdf_lut: texture_2d<f32>,
//     sampler_repeat: sampler, sampler_envmap: sampler,
// ) -> vec3f {
//     var base_color = textureSample(base_color_map, sampler_repeat, uv).rgb;
//     var roughness = textureSample(roughness_map, sampler_repeat, uv).r;
//     var metallic = textureSample(metallic_map, sampler_repeat, uv).r;

//     roughness = adjust(roughness, roughness_adjust);
//     metallic = adjust(metallic, metallic_adjust);

//     let normal_wn = normalize(normal_w);
//     let tangent_wn = normalize(tangent_w);

//     let N = apply_normal_map_amount(normal_map, uv, normal_wn, tangent_wn, normal_strength);
//     let V = normalize(camera_pos_w - pos_w);
//     let L = light_dir_wn;

//     var color_acc = vec3f(0.0);
//     color_acc += cook_torrance_brdf(V, N, L, base_color, metallic, roughness, light_color);
//     color_acc += cook_torrance_brdf_ibl(V, N, base_color, metallic, roughness, envmap, brdf_lut, ambient_color, sampler_envmap);

//     return color_acc;
// }

// @if(!ENTRY_POINT_FS_MAIN_NOOP) 
// fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {
//     var lightspace_pos = (u.g_light_projection_from_world * vec4<f32>(world_pos, 1.0)).xyz;
//     lightspace_pos = lightspace_pos * vec3f(0.5, -0.5, 1) + vec3f(0.5, 0.5, u.shadow_bias * -0.001);
//     return textureSampleCompare(shadow, sampler_shadow, lightspace_pos.xy, lightspace_pos.z);
// }

fn sample_shadow_map(world_pos: vec3<f32>, shadow: texture_depth_2d) -> f32 {
    var lightspace_pos = (u.g_light_projection_from_world * vec4<f32>(world_pos, 1.0)).xyz;
    lightspace_pos = lightspace_pos * vec3f(0.5, -0.5, 1) + vec3f(0.5, 0.5, u.shadow_bias * -0.001);
    return textureSampleCompare(shadow, sampler_shadow, lightspace_pos.xy, lightspace_pos.z);
}


@if(!ENTRY_POINT_FS_MAIN_NOOP) 
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let lightness = sample_shadow_map(in.v_pos_worldspace, shadow);

    let color = pbr_material(in.v_uv, in.v_pos_worldspace, in.v_normal_worldspace, 
        in.v_tangent_worldspace, 
        in.v_camera_pos_worldspace, u.g_light_dir_worldspace_norm,
        u.normal_strength, u.light_color.rgb * lightness, vec3f(u.ambient), 
        u.roughness, u.metallic, 
        base_color_map, roughness_map, metallic_map, normal_map, 
        envmap, brdf_lut, 
        sampler_repeat, sampler_envmap);
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_main_noop(in: VertexOutput) {}

fn get_particle_position(instance_index: u32) -> vec3<f32> {
    let current = particles_current[instance_index].position;
    let next = particles_next[instance_index].position;
    return mix(current, next, context.g_simulation_frame_ratio);
}

fn calculate_camera_pos_worldspace(camera_from_world: mat4x4<f32>) -> vec3<f32> {
    let myMat3x3 = mat3x3(camera_from_world[0].xyz, camera_from_world[1].xyz, camera_from_world[2].xyz);
    let inverse_rotation = transpose(myMat3x3);
    return inverse_rotation * -camera_from_world[3].xyz;
}

@vertex
fn vs_main(input: VertexInput, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let per_row = 8;
    let mi = vec3<f32>(f32(instance_index % u32(per_row)), f32(instance_index / u32(per_row)), 0.0);
    var mov = context.instance_move * (mi - vec3<f32>((f32(per_row) - 1.0) / 2.0, 0.0, 0.0));

    mov += get_particle_position(instance_index) * 200.0;

    output.v_pos_worldspace = (context.g_world_from_model * vec4<f32>(input.a_position, 1.0)).xyz + mov;

    output.position = context.g_projection_from_world * vec4<f32>(output.v_pos_worldspace, 1.0);

    output.v_uv = input.a_uv;
    output.v_normal_worldspace = (context.g_world_from_model * vec4<f32>(input.a_normal, 0.0)).xyz;
    output.v_tangent_worldspace = (context.g_world_from_model * vec4<f32>(input.a_tangent, 0.0)).xyz;
    output.v_camera_pos_worldspace = calculate_camera_pos_worldspace(context.g_camera_from_world);

    output.v_material_adjustment = vec3<f32>(0.99 - mi.x / (f32(per_row) - 1.0), mi.y / 2.0, 0.0);

    return output;
}
