pub mod shader;
pub mod shader_context;
pub mod vulkan_window;

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

enum MaterialPassEnum {
    EarlyDepth = 0,
    Shadow = 1,
    Solid = 2,
}
const MATERIAL_PASS_COUNT: usize = 3;

type VertexBuffer = CpuAccessibleBuffer<[Vertex3]>;
type Texture = ImageView<ImmutableImage>;

struct RenderItem {
    mesh: Mesh,
    position: Vec3,
    rotation: Vec3,
    material: [Option<MaterialPass>; MATERIAL_PASS_COUNT],
}

pub struct Drawable {
    pub pipeline: Arc<GraphicsPipeline>,
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex3]>>,
    pub uniform_buffer: CpuBufferPool<ContextUniforms>,
    pub texture: Arc<ImageView<ImmutableImage>>,
    pub descriptor_set: Arc<PersistentDescriptorSet>,
}

struct Mesh {
    vertex_buffer: Arc<VertexBuffer>,
}

struct MaterialPass {
    pipeline: Arc<GraphicsPipeline>,
    vertex_shader: Shader,
    fragment_shader: Shader,
    depth_test: bool,
    depth_write: bool,
}
