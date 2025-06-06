pub mod compute_call;
pub mod context;
pub mod double_buffer;
pub mod draw_call;
pub mod globals;
pub mod image;
pub mod mesh;
pub mod mipmap_generator;
pub mod shader;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3 {
    pub a_position: [f32; 3],
    pub a_normal: [f32; 3],
    pub a_tangent: [f32; 3],
    pub a_uv: [f32; 2],
    pub a_padding: f32,
}

const VERTEX_FORMAT: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x3,
    2 => Float32x3,
    3 => Float32x2,
    4 => Float32,
];

pub type MeshIndex = u32;

pub type Size2D = [u32; 2];
