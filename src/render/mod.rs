pub mod material;
pub mod mesh;
pub mod render_target;
pub mod render_unit;
pub mod shader;
pub mod shader_context;
pub mod vulkan_window;

use crate::render::material::Material;
use crate::render::mesh::Mesh;
use crate::render::shader::Shader;
use crate::render::shader_context::ContextUniforms;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::sync::Arc;
use vulkano::buffer::{CpuAccessibleBuffer, CpuBufferPool};
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::image::view::ImageView;
use vulkano::image::ImmutableImage;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::Subpass;

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

pub struct RenderObject {
    pub mesh: Mesh,
    pub position: Vec3,
    pub rotation: Vec3,
    pub material: Material,
}

pub struct Drawable {
    pub pipeline: Arc<GraphicsPipeline>,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex3]>>,
    pub uniform_buffer: CpuBufferPool<ContextUniforms>,
    pub texture: Arc<ImageView<ImmutableImage>>,
    pub descriptor_set: Arc<PersistentDescriptorSet>,
}
