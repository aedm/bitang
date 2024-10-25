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
    pop: f32,
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

@group(1) @binding(1) var envmap_sampler: sampler;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;
@group(1) @binding(3) var texture_sampler: sampler;

fn adjust(value: f32, factor: f32) -> f32 {
    if (factor < 0.0) {
        return value * (1.0 + factor);
    }
    return factor + value * (1.0 - factor);
}

fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {
    var lightspace_pos = (u.g_light_projection_from_world * vec4<f32>(world_pos, 1.0)).xyz;
    lightspace_pos.xy = lightspace_pos.xy * 0.5 + 0.5;
    lightspace_pos.z -= u.shadow_bias * 0.001;
    return textureSampleCompare(shadow, shadow_sampler, lightspace_pos.xy, lightspace_pos.z);
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.v_uv * 2.0;
    var base_color = textureSample(base_color_map, texture_sampler, uv).rgb;
    base_color = u.color;

    var roughness = textureSample(roughness_map, texture_sampler, uv).r;
    var metallic = textureSample(metallic_map, texture_sampler, uv).r;

    let light = sample_shadow_map(in.v_pos_worldspace);

    roughness = adjust(roughness, in.v_material_adjustment.x * 2.0 - 1.0);
    metallic = adjust(metallic, in.v_material_adjustment.y * 2.0 - 1.0);

    roughness = adjust(roughness, u.roughness);
    metallic = adjust(metallic, u.metallic);

    roughness = in.v_material_adjustment.x;
    metallic = in.v_material_adjustment.y;

    let normal_wn = normalize(in.v_normal_worldspace);
    let tangent_wn = normalize(in.v_tangent_worldspace);

    let N = apply_normal_map_amount(normal_map, uv, normal_wn, tangent_wn, u.normal_strength);
    let V = normalize(in.v_camera_pos_worldspace - in.v_pos_worldspace);
    let L = u.g_light_dir_worldspace_norm;

    base_color /= (u.pop + 1.0);
    var color_acc = vec3<f32>(0.0);
    color_acc += cook_torrance_brdf(V, N, L, base_color, metallic, roughness, u.light_color.rgb * light);
    color_acc += cook_torrance_brdf_ibl(V, N, base_color, metallic, roughness, envmap, brdf_lut, vec3<f32>(u.ambient * (u.pop + 1.0)));

    return vec4<f32>(color_acc, 1.0);
}
