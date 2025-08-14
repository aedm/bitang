struct Context {
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

@group(0) @binding(0) var<uniform> context: Context;

@group(0) @binding(1)
var<storage, read> particles_current: array<Particle>;

@group(0) @binding(2)
var<storage, read_write> particles_next: array<Particle>;

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
