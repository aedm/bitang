// Constants
// const PI: f32 = 3.1415926535;
// import super::external::PI;

// Struct for uniform buffer
struct Uniforms {
    g_app_time: f32,
    args: vec4<f32>,
    args2: vec4<f32>,
    col1: vec3<f32>,
    col2: vec3<f32>,
};

// Bindings
@group(1) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(1) var envmap: texture_2d<f32>;
@group(1) @binding(2) var sampler_envmap: sampler;

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

// Vertex Shader Output / Fragment Shader Input
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) v_ray_direction: vec3<f32>,
};

// Fragment Shader
@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = sample_environment_map(normalize(in.v_ray_direction), -1.0);
    return vec4<f32>(c.rgb * uniforms.args.r, 1.0);
}
