struct UniformsVs {
    g_projection_from_camera: mat4x4<f32>,
    g_camera_from_world: mat4x4<f32>,
};

// Struct for uniform buffer
struct UniformsFs {
    g_app_time: f32,
    args: vec4<f32>,
    args2: vec4<f32>,
    col1: vec3<f32>,
    col2: vec3<f32>,
};

// Bindings
@group(0) @binding(0) var<uniform> u: UniformsVs;
@group(1) @binding(0) var<uniform> uniforms: UniformsFs;
@group(1) @binding(1) var envmap: texture_2d<f32>;
@group(1) @binding(2) var sampler_envmap: sampler;

struct VertexInput {
    @location(0) a_position: vec3<f32>,
    @location(1) a_normal: vec3<f32>,
    @location(2) a_tangent: vec3<f32>,
    @location(3) a_uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) v_ray_direction: vec3<f32>,
};

fn calculate_backdrop_ray(uv: vec2<f32>) -> vec3<f32> {
    let fov = vec2<f32>(1.0 / u.g_projection_from_camera[0][0], 1.0 / u.g_projection_from_camera[1][1]);
    let inverse_rotation = transpose(mat3x3<f32>(
        u.g_camera_from_world[0].xyz,
        u.g_camera_from_world[1].xyz,
        u.g_camera_from_world[2].xyz
    ));
    return inverse_rotation * vec3<f32>(uv * fov, 1.0);
}

// Constants
const PI: f32 = 3.1415926535;

// Helper functions
fn direction_wn_to_spherical_envmap_uv(direction_wn: vec3<f32>) -> vec2<f32> {
    let phi = atan2(direction_wn.z, direction_wn.x);
    let theta = acos(direction_wn.y);

    let u = phi / (2.0 * PI) + 0.25;
    let v = theta / PI;

    return vec2<f32>(u, v);
}

fn sample_environment_map(direction_wn: vec3<f32>, bias: f32) -> vec4<f32> {
    let levels = textureNumLevels(envmap);
    let adjust = pow(1.0 - bias, 4.0);
    let mipLevel = max(f32(levels) - 3.5 - adjust * 7.0, 0.0);
    let uv = direction_wn_to_spherical_envmap_uv(direction_wn);
    return textureSampleLevel(envmap, sampler_envmap, uv, mipLevel);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.a_position.x, -in.a_position.z, 0.0, 1.0);
    let screen_uv = in.a_uv * 2.0 - 1.0;
    out.v_ray_direction = calculate_backdrop_ray(screen_uv);
    return out;
}

// Fragment Shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let c = sample_environment_map(normalize(in.v_ray_direction), -1.0);
    return vec4f(c.rgb * uniforms.args.r, 1.0);
}
