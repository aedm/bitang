// Constants
const PI: f32 = 3.14159265358979323846;

// Function to convert direction to spherical envmap UV coordinates
fn direction_wn_to_spherical_envmap_uv(direction_wn: vec3<f32>) -> vec2<f32> {
    let phi = atan2(direction_wn.z, direction_wn.x);
    let theta = acos(direction_wn.y);

    let u = phi / (2.0 * PI) + 0.25;
    let v = theta / PI;

    return vec2<f32>(u, v);
}

// Function to sample environment map
fn sample_environment_map(direction_wn: vec3<f32>, bias: f32, envmap: texture_2d<f32>) -> vec4<f32> {
    let levels = textureNumLevels(envmap);
    let adjust = pow(1.0 - bias, 4.0);
    let mipLevel = max(f32(levels) - 3.5 - adjust * 7.0, 0.0);
    let uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureSampleLevel(envmap, uv, mipLevel);
}

// Function to sample sRGB as linear
fn sample_srgb_as_linear(map: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
    let v = textureSample(map, uv).rgb;
    return pow(v, vec3<f32>(1.0 / 2.2));
}

// Function to apply normal map amount
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

// Function to make lightspace from worldspace transformation
fn make_lightspace_from_worldspace_transformation(light_dir_worldspace_n: vec3<f32>) -> mat3x3<f32> {
    let light_z = light_dir_worldspace_n;
    let light_x = normalize(cross(light_z, vec3<f32>(0.0, 1.0, 0.0)));
    let light_y = cross(light_x, light_z);

    let world_from_light = mat3x3<f32>(
        light_x,
        light_y,
        light_z
    );

    return transpose(world_from_light);
}
