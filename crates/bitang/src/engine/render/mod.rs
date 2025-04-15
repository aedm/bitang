pub mod double_buffer;
pub mod compute_call;
pub mod image;
pub mod mesh;
pub mod shader;
pub mod mipmap_generator;
pub mod draw_call;

use image::PixelFormat;

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

// TODO: move up one directory
pub const SCREEN_RENDER_TARGET_ID: &str = "screen";
pub const FRAMEDUMP_PIXEL_FORMAT: PixelFormat = PixelFormat::Rgba8U;

/// How many times the simulation is updated per second.
/// Weird number on purpose.
const SIMULATION_FREQUENCY_HZ: f32 = 60.0;
pub const SIMULATION_STEP_SECONDS: f32 = 1.0 / SIMULATION_FREQUENCY_HZ;
