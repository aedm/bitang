struct Uniforms {
    g_z_near: f32,
    g_app_time: f32,
    g_pixel_size: f32,
    g_camera_from_world: mat4x4<f32>,
    focus_distance: f32,
    focus_scale: f32,
    _pad: vec4f,
}

@group(1) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(1) var color_texture: texture_2d<f32>;
@group(1) @binding(2) var depth_texture: texture_depth_2d;

@group(1) @binding(3) var sampler_clamp_to_edge: sampler;


const GOLDEN_ANGLE: f32 = 2.39996323;
const MAX_BLUR_SIZE: f32 = 20.0;
const RAD_SCALE: f32 = 3.0; // Smaller = nicer blur, larger = faster
const FILM_GRAIN: f32 = 0.0;

fn getBlurSize(depth: f32, focusPoint: f32, focusScale: f32) -> f32 {
    let coc = clamp((1.0 / focusPoint - 1.0 / depth) * focusScale, -1.0, 1.0);
    return abs(coc) * MAX_BLUR_SIZE;
}

fn depth_sample_to_z2(buffer_value: f32) -> f32 {
    return uniforms.g_z_near / (1.0 - buffer_value);
}

fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn depthOfField(texCoord: vec2<f32>, focusPoint: f32, focusScale: f32) -> vec3<f32> {
    let centerDepth = depth_sample_to_z2(textureSample(depth_texture, sampler_clamp_to_edge, texCoord));
    let centerSize = getBlurSize(centerDepth, focusPoint, focusScale);
    var color = textureSample(color_texture, sampler_clamp_to_edge, texCoord).rgb;
    var tot = 1.0;
    var radius = RAD_SCALE;

    let an = fract(rand(texCoord + fract(uniforms.g_app_time)) * FILM_GRAIN);

    for(var ang = an; radius < MAX_BLUR_SIZE; radius += RAD_SCALE/radius) {
        let tc = texCoord + vec2<f32>(cos(ang), sin(ang)) * uniforms.g_pixel_size * radius;
        let sampleColor = min(textureSample(color_texture, sampler_clamp_to_edge, tc).rgb, vec3<f32>(1.0));
        let sampleDepth = depth_sample_to_z2(textureSample(depth_texture, sampler_clamp_to_edge, tc));

        var sampleSize = getBlurSize(sampleDepth, focusPoint, focusScale);
        if (sampleDepth > centerDepth) {
            sampleSize = clamp(sampleSize, 0.0, centerSize * 2.0);
        }
        let m = smoothstep(radius - 0.5, radius + 0.5, sampleSize);
        color += mix(color/tot, sampleColor, m);
        tot += 1.0;
        ang += GOLDEN_ANGLE;
    }
    return color / tot;
}

struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = uniforms.g_camera_from_world * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return vec4<f32>(depthOfField(in.v_uv, uniforms.focus_distance, uniforms.focus_scale), 1.0);
}
