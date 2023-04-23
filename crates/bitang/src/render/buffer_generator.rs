use crate::control::controls::{Control, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use glam::Vec3;
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
        let vec4_size = self.size as usize * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b, c, dt] = self.y.as_vec4().as_ref();
        let mut p = self.x.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..self.size {
            let np = Vec3::new(
                p.x + dt * a * (p.y - p.x),
                p.y + dt * (p.x * (b - p.z) - p.y),
                p.z + dt * (p.x * p.y - c * p.z),
            );
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np; // TODO: prove it

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        let buffer = self.buffer_pool.from_iter(data)?;
        *self.current_buffer.borrow_mut() = Some(buffer);
        Ok(())
    }

    pub fn get_buffer(&self) -> Option<Arc<CpuBufferPoolChunk<BufferItem>>> {
        self.current_buffer.borrow().clone()
    }
}
