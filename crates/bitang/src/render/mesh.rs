use crate::render::vulkan_window::VulkanContext;
use crate::render::Vertex3;
use anyhow::Result;
use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};

pub type VertexBuffer = CpuAccessibleBuffer<[Vertex3]>;

#[derive(Clone)]
pub struct Mesh {
    // TODO: omit Arc
    pub vertex_buffer: Arc<VertexBuffer>,
}

impl Mesh {
    pub fn try_new(context: &VulkanContext, vertices: Vec<Vertex3>) -> Result<Mesh> {
        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            context.context.memory_allocator(),
            BufferUsage {
                vertex_buffer: true,
                ..BufferUsage::empty()
            },
            false,
            vertices,
        )?;
        Ok(Mesh { vertex_buffer })
    }
}