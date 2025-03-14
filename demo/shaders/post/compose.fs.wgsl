const GAMMA: f32 = 2.2;
const COLOR_BASE_LEVEL: f32 = 1.0;

struct Uniforms {
    g_pixel_size: vec2<f32>,
    tone_mapping: vec3<f32>,
    coloring: vec3<f32>,
    glow: f32,
    glow_pow: f32,
    g_app_time: f32,
    hdr_adjust: f32,
}

@group(1) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(1) var base_color: texture_2d<f32>;
@group(1) @binding(2) var glow_map: texture_2d<f32>;
@group(1) @binding(3) var sampler_clamp_to_edge: sampler;

fn gamma_compress(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / GAMMA));
}

fn gamma_decompress(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(GAMMA));
}

fn noise2d(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // base color
    var color = textureSampleLevel(base_color, sampler_clamp_to_edge, in.v_uv, 0.0).rgb;
    color = min(color, vec3<f32>(1.0));

    // glow
    // #ifdef IMAGE_BOUND_TO_SAMPLER_GLOW_MAP
    var glow_color = textureSampleLevel(glow_map, sampler_clamp_to_edge, in.v_uv, 0.0).rgb;
    glow_color = gamma_decompress(glow_color);
    glow_color = pow(glow_color, vec3<f32>(uniforms.glow_pow)) * uniforms.glow;
    color += glow_color;
    // #endif

    // tone mapping
    color = pow(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), uniforms.tone_mapping) * uniforms.coloring;

    // hdr
    color = color / (color + vec3<f32>(1.0 - uniforms.hdr_adjust));

    // dither
    let dither = noise2d(in.v_uv * 512.0 + uniforms.g_app_time * 0.0) / 512.0;
    color += vec3<f32>(dither);

    return vec4<f32>(color, 1.0);
}
