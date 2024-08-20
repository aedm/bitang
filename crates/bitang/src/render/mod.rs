pub mod buffer;
pub mod buffer_generator;
pub mod camera;
pub mod chart;
pub mod compute;
pub mod draw;
pub mod generate_mip_levels;
pub mod image;
pub mod material;
pub mod mesh;
pub mod pass;
pub mod project;
pub mod render_object;
pub mod scene;
pub mod shader;

use crate::render::image::PixelFormat;
use vulkano::{buffer::BufferContents, pipeline::graphics::vertex_input::Vertex};

#[derive(BufferContents, Vertex, Default, Clone, Copy, Debug)]
#[repr(C)]
pub struct Vertex3 {
    #[format(R32G32B32_SFLOAT)]
    pub a_position: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    pub a_normal: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    pub a_tangent: [f32; 3],
    #[format(R32G32_SFLOAT)]
    pub a_uv: [f32; 2],
    #[format(R32_SFLOAT)]
    pub a_padding: f32,
}

pub const SCREEN_COLOR_FORMAT: PixelFormat = PixelFormat::Bgra8Srgb;
pub const SCREEN_RENDER_TARGET_ID: &str = "screen";

type BufferItem = [f32; 4];

/// How many times the simulation is updated per second.
/// Weird number on purpose.
const SIMULATION_FREQUENCY_HZ: f32 = 60.0;
pub const SIMULATION_STEP_SECONDS: f32 = 1.0 / SIMULATION_FREQUENCY_HZ;
