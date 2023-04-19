pub mod chart;
pub mod material;
pub mod mesh;
pub mod project;
pub mod render_target;
pub mod render_unit;
pub mod vulkan_window;

use std::rc::Rc;
use crate::render::material::Material;
use crate::render::mesh::Mesh;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::sync::Arc;
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use crate::control::controls::Control;

#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex3 {
    pub a_position: [f32; 3],
    pub a_normal: [f32; 3],
    pub a_tangent: [f32; 3],
    pub a_uv: [f32; 2],
    pub a_padding: f32,
}

vulkano::impl_vertex!(Vertex3, a_position, a_normal, a_tangent, a_uv, a_padding);

pub type Texture = ImageView<ImmutableImage>;

#[derive(Clone)]
pub struct RenderObject {
    pub id: String,
    pub mesh: Arc<Mesh>,
    pub material: Material,
    pub position: Rc<Control>,
    pub rotation: Rc<Control>,
}
