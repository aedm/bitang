const GAMMA: f32 = 2.2;
const COLOR_BASE_LEVEL: f32 = 1.0;

fn gamma_compress(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / GAMMA));
}

fn gamma_decompress(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(GAMMA));
}

@group(1) @binding(1) var base_color: texture_2d<f32>;
@group(1) @binding(3) var sampler_clamp_to_edge: sampler;

struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSampleLevel(base_color, sampler_clamp_to_edge, in.v_uv, 0.0).rgb;
    color = min(color, vec3<f32>(4.0));
    color = max(color - vec3<f32>(COLOR_BASE_LEVEL), vec3<f32>(0.0));
    color = gamma_compress(color);
    return vec4<f32>(color, 1.0);
}
