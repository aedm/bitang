use crate::{render::BufferItem, tool::GpuContext};
use crate::tool::WindowContext;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
// use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
// use vulkano::buffer::{BufferUsage, Subbuffer};


// pub struct Subbuffers {
//     // pub current: Subbuffer<[BufferItem]>,
//     // pub next: Subbuffer<[BufferItem]>,
//     pub current: wgpu::Buffer,
//     pub next: wgpu::Buffer,
// }

// TODO: id
pub struct Buffer {
    pub item_size_in_vec4: usize,
    pub item_count: usize,
    // buffer_pool: SubbufferAllocator,
    // pub buffers: RwLock<Subbuffers>,
    pub current: RefCell<Rc<wgpu::Buffer>>,
    pub next: RefCell<Rc<wgpu::Buffer>>,

}

impl Buffer {
    pub fn new(context: &GpuContext, item_size_in_vec4: usize, item_count: usize) -> Self {
        // let buffer_pool = SubbufferAllocator::new(
        //     context.memory_allocator.clone(),
        //     SubbufferAllocatorCreateInfo {
        //         buffer_usage: BufferUsage::STORAGE_BUFFER,
        //         ..Default::default()
        //     },
        // );

        let size = item_count * item_size_in_vec4 * size_of::<glam::Vec4>();
        let current = context.device.create_buffer(&wgpu::BufferDescriptor {
            // TODO: id
            label: None,
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let next = context.device.create_buffer(&wgpu::BufferDescriptor {
            // TODO: id
            label: None,
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Buffer {
            item_size_in_vec4,
            item_count,
            current: RefCell::new(Rc::new(current)),
            next: RefCell::new(Rc::new(next)),
        }
    }

    pub fn step(&self) {
        let mut current = self.current.borrow_mut();
        let mut next = self.next.borrow_mut();
        std::mem::swap(&mut *current, &mut *next);
        // let next =
        //     Self::allocate_buffer(&self.buffer_pool, self.item_size_in_vec4, self.item_count);
        // let mut buffers = self.buffers.write().unwrap();
        // std::mem::swap(&mut buffers.current, &mut buffers.next);
        // *buffers = Subbuffers {
        //     current: buffers.next.clone(),
        //     next,
        // };
    }

    pub fn get_current_buffer(&self) -> Rc<wgpu::Buffer> {
        self.current.borrow().clone()
    }

    pub fn get_next_buffer(&self) -> Rc<wgpu::Buffer> {
        self.next.borrow().clone()
    }
}
