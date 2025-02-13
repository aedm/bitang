use crate::render::BufferItem;
use crate::tool::RenderContext;
use std::sync::{Arc, RwLock};
// use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
// use vulkano::buffer::{BufferUsage, Subbuffer};


pub struct Subbuffers {
    // pub current: Subbuffer<[BufferItem]>,
    // pub next: Subbuffer<[BufferItem]>,
    pub current: wgpu::Buffer,
    pub next: wgpu::Buffer,
}

pub struct Buffer {
    pub item_size_in_vec4: usize,
    pub item_count: usize,
    // buffer_pool: SubbufferAllocator,
    pub buffers: RwLock<Subbuffers>,
}

impl Buffer {
    pub fn new(context: &RenderContext, item_size_in_vec4: usize, item_count: usize) -> Self {
        // let buffer_pool = SubbufferAllocator::new(
        //     context.memory_allocator.clone(),
        //     SubbufferAllocatorCreateInfo {
        //         buffer_usage: BufferUsage::STORAGE_BUFFER,
        //         ..Default::default()
        //     },
        // );

        let subbuffers = Subbuffers {
            current: Self::allocate_buffer(&buffer_pool, item_size_in_vec4, item_count),
            next: Self::allocate_buffer(&buffer_pool, item_size_in_vec4, item_count),
        };

        Buffer {
            item_size_in_vec4,
            item_count,
            buffer_pool,
            buffers: RwLock::new(subbuffers),
        }
    }

    fn allocate_buffer(
        buffer_pool: &SubbufferAllocator,
        item_size_in_vec4: usize,
        item_count: usize,
    ) -> Subbuffer<[BufferItem]> {
        let buffer_size_in_vec4 = item_count * item_size_in_vec4;
        buffer_pool
            .allocate_slice(buffer_size_in_vec4 as _)
            .unwrap()
    }

    pub fn step(&self) {
        let next =
            Self::allocate_buffer(&self.buffer_pool, self.item_size_in_vec4, self.item_count);
        let mut buffers = self.buffers.write().unwrap();
        *buffers = Subbuffers {
            current: buffers.next.clone(),
            next,
        };
    }

    pub fn get_current_buffer(&self) -> Subbuffer<[BufferItem]> {
        self.buffers.read().unwrap().current.clone()
    }

    pub fn get_next_buffer(&self) -> Subbuffer<[BufferItem]> {
        self.buffers.read().unwrap().next.clone()
    }
}
