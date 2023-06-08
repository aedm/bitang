pub mod buffer_generator;
pub mod camera;
pub mod chart;
pub mod draw;
pub mod material;
pub mod mesh;
pub mod project;
pub mod render_target;
pub mod render_unit;
pub mod vulkan_window;

use crate::control::controls::Control;
use crate::render::material::Material;
use crate::render::mesh::Mesh;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
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

pub type Texture = ImageView<ImmutableImage>;

#[derive(Clone)]
pub struct RenderObject {
    pub id: String,
    pub mesh: Arc<Mesh>,
    pub material: Material,
    pub position: Rc<Control>,
    pub rotation: Rc<Control>,
    pub instances: Rc<Control>,
}

pub const DEPTH_BUFFER_FORMAT: Format = Format::D32_SFLOAT;
pub const SCREEN_COLOR_FORMAT: Format = Format::B8G8R8A8_SRGB;
