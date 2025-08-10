struct Uniforms {
    g_projection_from_camera: mat4x4<f32>,
    g_camera_from_world: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

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
    let inverse_rotation = mat3x3<f32>(
        u.g_camera_from_world[0].xyz,
        u.g_camera_from_world[1].xyz,
        u.g_camera_from_world[2].xyz
    );
    return inverse_rotation * vec3<f32>(uv * fov, 1.0);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.a_position.x, -in.a_position.z, 0.0, 1.0);
    let screen_uv = in.a_uv * 2.0 - 1.0;
    out.v_ray_direction = calculate_backdrop_ray(screen_uv);
    return out;
}
