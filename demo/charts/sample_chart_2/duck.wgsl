import package::shaders::pbr::pbr_material;
import package::shaders::quaternion::{q_slerp, q_rotate, Quaternion};
import package::shaders::math::calculate_camera_pos_worldspace;
import super::particle::Particle;

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

fn get_particle_rotation(instance_index: u32) -> Quaternion {
    let current = particles_current[instance_index].rotation_quat;
    let next = particles_next[instance_index].rotation_quat;
    return q_slerp(current, next, context.g_simulation_frame_ratio);
}

@vertex
fn vs_main(input: VertexInput, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let per_row = 8;
    let mi = vec3<f32>(f32(instance_index % u32(per_row)), f32(instance_index / u32(per_row)), 0.0);
    var mov = context.instance_move * (mi - vec3<f32>((f32(per_row) - 1.0) / 2.0, 0.0, 0.0));

    mov += get_particle_position(instance_index) * 200.0;

    let r = get_particle_rotation(instance_index);
    let position = q_rotate(input.a_position, r);
    let normal = q_rotate(input.a_normal, r);
    let tangent = q_rotate(input.a_tangent, r);

    output.v_pos_worldspace = (context.g_world_from_model * vec4<f32>(position, 1.0)).xyz + mov;

    output.position = context.g_projection_from_world * vec4<f32>(output.v_pos_worldspace, 1.0);

    output.v_uv = input.a_uv;
    output.v_normal_worldspace = (context.g_world_from_model * vec4<f32>(normal, 0.0)).xyz;
    output.v_tangent_worldspace = (context.g_world_from_model * vec4<f32>(tangent, 0.0)).xyz;
    output.v_camera_pos_worldspace = calculate_camera_pos_worldspace(context.g_camera_from_world);

    output.v_material_adjustment = vec3<f32>(0.99 - mi.x / (f32(per_row) - 1.0), mi.y / 2.0, 0.0);

    return output;
}
