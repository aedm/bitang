use super::context::GpuContext;
use std::array::from_fn;
use std::cell::Cell;

// TODO: id
pub struct DoubleBuffer {
    #[allow(dead_code)]
    pub item_size_in_vec4: usize,
    pub item_count: usize,
    buffers: [wgpu::Buffer; 2],
    current_index: Cell<usize>,
}

impl DoubleBuffer {
    pub fn new(context: &GpuContext, item_size_in_vec4: usize, item_count: usize) -> Self {
        let size = (item_count * item_size_in_vec4 * size_of::<glam::Vec4>()) as u64;
        let buffers = from_fn(|_| {
            context.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        DoubleBuffer {
            item_size_in_vec4,
            item_count,
            buffers,
            current_index: Cell::new(0),
        }
    }

    pub fn step(&self) {
        self.current_index.set(1 - self.current_index.get());
    }

    pub fn get_current_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.current_index.get()]
    }

    pub fn get_next_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[1 - self.current_index.get()]
    }
}
