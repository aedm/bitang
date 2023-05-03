use crate::control::controls::{Control, GlobalType};
use crate::render::buffer_generator::BufferGenerator;
use crate::render::render_target::RenderTarget;
use crate::render::Texture;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use vulkano::sampler::SamplerAddressMode;
use vulkano::shader::ShaderModule;

#[derive(Copy, Clone, Debug)]
pub enum MaterialStepType {
    _EarlyDepth = 0,
    _Shadow = 1,
    Solid = 2,
}
pub const MATERIAL_STEP_COUNT: usize = 3;

#[derive(Clone)]
pub struct Material {
    pub passes: [Option<MaterialStep>; MATERIAL_STEP_COUNT],
    pub sampler_address_mode: SamplerAddressMode,
}

#[derive(Clone)]
pub enum BlendMode {
    None,
    Alpha,
    Additive,
}

#[derive(Clone)]
pub struct MaterialStep {
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_mode: BlendMode,
    pub sampler_address_mode: SamplerAddressMode,
}

pub enum ShaderKind {
    Vertex = 0,
    Fragment = 1,
}

#[derive(Clone)]
pub struct Shader {
    pub shader_module: Arc<ShaderModule>,
    pub descriptor_bindings: Vec<DescriptorBinding>,
    pub global_uniform_bindings: Vec<GlobalUniformMapping>,
    pub local_uniform_bindings: Vec<LocalUniformMapping>,
    pub uniform_buffer_size: usize,
}

#[derive(Clone)]
pub enum DescriptorSource {
    Texture(Arc<Texture>),
    RenderTarget(Arc<RenderTarget>),
    BufferGenerator(Arc<BufferGenerator>),
}

#[derive(Clone)]
pub struct DescriptorBinding {
    pub descriptor_source: DescriptorSource,
    pub descriptor_set_binding: u32,
}

#[derive(Clone)]
pub struct LocalUniformMapping {
    pub control: Rc<Control>,
    pub f32_count: usize,
    pub f32_offset: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct GlobalUniformMapping {
    pub global_type: GlobalType,
    pub offset: usize,
}
