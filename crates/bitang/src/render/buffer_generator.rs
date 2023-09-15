use crate::control::controls::{Control, ControlSetBuilder};
use crate::control::{ControlId, ControlIdPartType};
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use glam::Vec3;
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::{BufferUsage, Subbuffer};

type BufferItem = [f32; 4];

#[derive(Debug, Deserialize)]
pub enum BufferGeneratorType {
    Lorenz,
    Roessler,
    Thomas,
    Aizawa,
    Dadras,
    RabinovichFabrikant,
}

trait BufferGeneratorImpl {
    fn generate(&self, size: usize) -> Vec<BufferItem>;
}

pub struct BufferGenerator {
    size: u32,
    buffer_pool: SubbufferAllocator,
    pub current_buffer: RefCell<Option<Subbuffer<[BufferItem]>>>,
    generator: Rc<dyn BufferGeneratorImpl>,
}

impl BufferGenerator {
    pub fn new(
        size: u32,
        context: &VulkanContext,
        control_id: &ControlId,
        control_set_builder: &mut ControlSetBuilder,
        generator_type: &BufferGeneratorType,
    ) -> Self {
        let buffer_pool = SubbufferAllocator::new(
            context.vulkano_context.memory_allocator().clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
        );

        let generator: Rc<dyn BufferGeneratorImpl> = match generator_type {
            BufferGeneratorType::Lorenz => {
                Rc::new(LorenzGenerator::new(control_id, control_set_builder))
            }
            BufferGeneratorType::Roessler => {
                Rc::new(RoesslerGenerator::new(control_id, control_set_builder))
            }
            BufferGeneratorType::Thomas => {
                Rc::new(ThomasGenerator::new(control_id, control_set_builder))
            }
            BufferGeneratorType::Aizawa => {
                Rc::new(AizawaGenerator::new(control_id, control_set_builder))
            }
            BufferGeneratorType::Dadras => {
                Rc::new(DadrasGenerator::new(control_id, control_set_builder))
            }
            BufferGeneratorType::RabinovichFabrikant => Rc::new(RabinovichFabrikantGenerator::new(
                control_id,
                control_set_builder,
            )),
        };

        BufferGenerator {
            size,
            buffer_pool,
            current_buffer: RefCell::new(None),
            generator,
        }
    }

    pub fn generate(&self) -> Result<()> {
        let data = self.generator.generate(self.size as usize);
        let buffer = self.buffer_pool.allocate_slice(data.len() as _)?;
        buffer.write()?.copy_from_slice(&data);
        *self.current_buffer.borrow_mut() = Some(buffer);
        Ok(())
    }

    pub fn get_buffer(&self) -> Option<Subbuffer<[BufferItem]>> {
        self.current_buffer.borrow().clone()
    }
}

struct LorenzGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params: Rc<Control>,
}

impl LorenzGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_id = control_id.add(ControlIdPartType::Value, "lorenz-params");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[0.1, 0.0, 0.0]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params: control_set_builder.get_vec3_with_default(&params_id, &[10.0, 28.0, 8.0 / 3.0]),
        }
    }
}

impl BufferGeneratorImpl for LorenzGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b, c] = self.params.as_vec3().as_ref();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np =
                p + Vec3::new(a * (p.y - p.x), p.x * (b - p.z) - p.y, p.x * p.y - c * p.z) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}

struct RoesslerGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params: Rc<Control>,
}

impl RoesslerGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_id = control_id.add(ControlIdPartType::Value, "roessler-params");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[0.1, 0.0, 0.0]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params: control_set_builder.get_vec3_with_default(&params_id, &[0.2, 0.2, 5.7]),
        }
    }
}

impl BufferGeneratorImpl for RoesslerGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b, c] = self.params.as_vec3().as_ref();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np = p + Vec3::new(-p.y - p.z, p.x + a * p.y, b + p.z * (p.x - c)) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}

struct ThomasGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params: Rc<Control>,
}

impl ThomasGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_id = control_id.add(ControlIdPartType::Value, "thomas-params");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[0.1, 0.0, 0.0]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params: control_set_builder.get_float_with_default(&params_id, 0.208186),
        }
    }
}

impl BufferGeneratorImpl for ThomasGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let b = self.params.as_float();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np = p + Vec3::new(
                p.y.sin() - b * p.x,
                p.z.sin() - b * p.y,
                p.x.sin() - b * p.z,
            ) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}

struct AizawaGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params_1: Rc<Control>,
    params_2: Rc<Control>,
}

impl AizawaGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_1_id = control_id.add(ControlIdPartType::Value, "aizawa-params-1");
        let params_2_id = control_id.add(ControlIdPartType::Value, "aizawa-params-2");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[0.1, 0.0, 0.0]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params_1: control_set_builder.get_vec3_with_default(&params_1_id, &[0.95, 0.7, 0.6]),
            params_2: control_set_builder.get_vec3_with_default(&params_2_id, &[3.5, 0.25, 0.1]),
        }
    }
}

impl BufferGeneratorImpl for AizawaGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b, c] = self.params_1.as_vec3().as_ref();
        let &[d, e, f] = self.params_2.as_vec3().as_ref();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np = p + Vec3::new(
                (p.z - b) * p.x - d * p.y,
                d * p.x + (p.z - b) * p.y,
                c + a * p.z - (p.z * p.z * p.z) / 3.0 - (p.x * p.x + p.y * p.y) * (1.0 + e * p.z)
                    + f * p.z * p.x * p.x * p.x,
            ) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}

struct DadrasGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params_1: Rc<Control>,
    params_2: Rc<Control>,
}

impl DadrasGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_1_id = control_id.add(ControlIdPartType::Value, "dadras-params-1");
        let params_2_id = control_id.add(ControlIdPartType::Value, "dadras-params-2");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[1.0, 1.0, 1.9]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params_1: control_set_builder.get_vec3_with_default(&params_1_id, &[3.0, 2.7, 1.7]),
            params_2: control_set_builder.get_vec2_with_default(&params_2_id, &[2.0, 9.0]),
        }
    }
}

impl BufferGeneratorImpl for DadrasGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b, c] = self.params_1.as_vec3().as_ref();
        let &[d, e] = self.params_2.as_vec2().as_ref();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np = p + Vec3::new(
                p.y - a * p.x + b * p.y * p.z,
                c * p.y - p.x * p.z + p.z,
                d * p.x * p.y - e * p.z,
            ) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}

struct RabinovichFabrikantGenerator {
    init: Rc<Control>,
    delta: Rc<Control>,
    params: Rc<Control>,
}

impl RabinovichFabrikantGenerator {
    fn new(control_id: &ControlId, control_set_builder: &mut ControlSetBuilder) -> Self {
        let init_id = control_id.add(ControlIdPartType::Value, "init");
        let delta_id = control_id.add(ControlIdPartType::Value, "delta");
        let params_id = control_id.add(ControlIdPartType::Value, "rabinovich-fabrikant-params");
        Self {
            init: control_set_builder.get_vec3_with_default(&init_id, &[0.1, 0.0, 0.0]),
            delta: control_set_builder.get_float_with_default(&delta_id, 0.1),
            params: control_set_builder.get_vec2_with_default(&params_id, &[0.14, 0.1]),
        }
    }
}

impl BufferGeneratorImpl for RabinovichFabrikantGenerator {
    fn generate(&self, size: usize) -> Vec<BufferItem> {
        let vec4_size = size * 3;
        let mut data = Vec::with_capacity(vec4_size);

        let &[a, b] = self.params.as_vec2().as_ref();
        let dt = self.delta.as_float() * 0.01;
        let mut p = self.init.as_vec3();
        let mut normal = Vec3::new(0.0, 1.0, 0.0);

        for _ in 0..size {
            let np = p + Vec3::new(
                p.y * (p.z - 1.0 + p.x * p.x) + b * p.x,
                p.x * (3.0 * p.z + 1.0 - p.x * p.x) + b * p.y,
                -2.0 * p.z * (a + p.x * p.y),
            ) * dt;
            let tangent = (np - p).normalize();
            normal = tangent.cross(normal.cross(tangent).normalize());
            p = np;

            data.push([p.x, p.y, p.z, 0.0]);
            data.push([normal.x, normal.y, normal.z, 0.0]);
            data.push([tangent.x, tangent.y, tangent.z, 0.0]);
        }
        data
    }
}
