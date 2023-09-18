use crate::render::vulkan_window::VulkanContext;
use crate::render::Vertex3;
use anyhow::Result;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};

pub type VertexBuffer = Subbuffer<[Vertex3]>;

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffer: VertexBuffer,
}

impl Mesh {
    pub fn try_new(context: &Arc<VulkanContext>, vertices: Vec<Vertex3>) -> Result<Mesh> {
        let vertex_buffer = Buffer::from_iter(
            context.vulkano_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                usage: MemoryUsage::Upload,
                ..Default::default()
            },
            vertices,
        )?;
        Ok(Mesh { vertex_buffer })
    }
}
