use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolChunk;
use vulkano::buffer::CpuBufferPool;

type BufferItem = [f32; 4];

pub struct BufferGenerator {
    size: u32,
    buffer_pool: CpuBufferPool<BufferItem>,
}

impl BufferGenerator {
    pub fn new(size: u32, context: &VulkanContext) -> Self {
        let buffer_pool = CpuBufferPool::vertex_buffer(context.context.memory_allocator().clone());
        BufferGenerator { size, buffer_pool }
    }

    fn generate(&mut self) -> Result<Arc<CpuBufferPoolChunk<BufferItem>>> {
        let mut data = Vec::with_capacity(self.size as usize);
        for i in 0..self.size {
            let x = i as f32 / self.size as f32;
            data.push([x.sin(), x.cos(), x, 0.0]);
        }
        let buffer = self.buffer_pool.from_iter(data)?;
        Ok(buffer)
    }
}
