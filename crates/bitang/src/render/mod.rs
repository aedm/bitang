pub mod buffer_generator;
pub mod camera;
pub mod chart;
pub mod draw;
pub mod image;
pub mod material;
pub mod mesh;
pub mod pass;
pub mod project;
pub mod render_object;
pub mod shader;
pub mod vulkan_window;

use crate::render::image::ImageFormat;
use vulkano::{buffer::BufferContents, pipeline::graphics::vertex_input::Vertex};

#[derive(BufferContents, Vertex)]
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

pub const DEPTH_BUFFER_FORMAT: ImageFormat = ImageFormat::Depth32F;
pub const SCREEN_COLOR_FORMAT: ImageFormat = ImageFormat::Rgba8Srgb;
pub const SCREEN_RENDER_TARGET_ID: &str = "screen";
pub const SCREEN_DEPTH_RENDER_TARGET_ID: &str = "screen_depth";
