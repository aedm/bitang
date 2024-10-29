struct VertexOutput {
    @location(0) v_uv: vec2<f32>,
    @location(1) v_normal_worldspace: vec3<f32>,
    @location(2) v_tangent_worldspace: vec3<f32>,
    @location(3) v_pos_worldspace: vec3<f32>,
    @location(4) v_camera_pos_worldspace: vec3<f32>,
    @location(5) v_material_adjustment: vec3<f32>,
};

@fragment
fn main(in: VertexOutput) {}
