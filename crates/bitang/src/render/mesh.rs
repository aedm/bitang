use crate::render::Vertex3;
use crate::tool::VulkanContext;
use anyhow::Result;
use std::sync::Arc;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, IndexBuffer, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};

pub type VertexBuffer = Subbuffer<[Vertex3]>;

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffer: VertexBuffer,
    pub index_buffer: Option<IndexBuffer>,
}

impl Mesh {
    pub fn try_new(
        context: &Arc<VulkanContext>,
        vertices: Vec<Vertex3>,
        indices: Option<Vec<u32>>,
    ) -> Result<Mesh> {
        let vertex_buffer = Buffer::from_iter(
            context.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices,
        )?;
        let index_buffer = if let Some(indices) = indices {
            Some(
                Buffer::from_iter(
                    context.memory_allocator.clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    indices,
                )?
                .into(),
            )
        } else {
            None
        };
        Ok(Mesh {
            vertex_buffer,
            index_buffer,
        })
    }
}
