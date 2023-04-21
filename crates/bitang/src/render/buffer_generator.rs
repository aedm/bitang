use crate::control::controls::{Control, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolChunk;
use vulkano::buffer::{BufferUsage, CpuBufferPool};
use vulkano::memory::allocator::MemoryUsage;

type BufferItem = [f32; 4];

pub struct BufferGenerator {
    size: u32,
    buffer_pool: CpuBufferPool<BufferItem>,
    current_buffer: RefCell<Option<Arc<CpuBufferPoolChunk<BufferItem>>>>,
    x: Rc<Control>,
    y: Rc<Control>,
}

impl BufferGenerator {
    pub fn new(
        size: u32,
        context: &VulkanContext,
        control_id: &ControlId,
        control_set_builder: &mut ControlSetBuilder,
    ) -> Self {
        let x_id = control_id.add(ControlIdPartType::Value, "x");
        let y_id = control_id.add(ControlIdPartType::Value, "y");

        let buffer_pool = CpuBufferPool::new(
            context.context.memory_allocator().clone(),
            BufferUsage {
                storage_buffer: true,
                ..BufferUsage::empty()
            },
            MemoryUsage::Upload,
        );
        BufferGenerator {
            size,
            buffer_pool,
            current_buffer: RefCell::new(None),
            x: control_set_builder.get_control_with_default(&x_id, &[0.1, 0.0, 0.0, 0.0]),
            y: control_set_builder.get_control_with_default(&y_id, &[10.0, 28.0, 8.0 / 3.0, 0.1]),
        }
    }

    pub fn generate(&self) -> Result<()> {
        let &[mut x, mut y, mut z] = self.x.as_vec3().as_ref();
        let &[a, b, c, dt] = self.y.as_vec4().as_ref();
        let mut data = Vec::with_capacity(self.size as usize);
        for _ in 0..self.size {
            let xt = x + dt * a * (y - x);
            let yt = y + dt * (x * (b - z) - y);
            let zt = z + dt * (x * y - c * z);
            x = xt;
            y = yt;
            z = zt;
            data.push([x, y, z, 0.0]);
        }
        let buffer = self.buffer_pool.from_iter(data)?;
        *self.current_buffer.borrow_mut() = Some(buffer);
        Ok(())
    }

    pub fn get_buffer(&self) -> Option<Arc<CpuBufferPoolChunk<BufferItem>>> {
        self.current_buffer.borrow().clone()
    }
}
