use crate::control::controls::{Control, ControlValue, GlobalType};
use crate::render::Texture;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::shader::ShaderModule;

pub enum MaterialStepType {
    EarlyDepth = 0,
    Shadow = 1,
    Solid = 2,
}
pub const MATERIAL_STEP_COUNT: usize = 3;

#[derive(Clone)]
pub struct Material {
    pub passes: [Option<MaterialStep>; MATERIAL_STEP_COUNT],
}

#[derive(Clone)]
pub struct MaterialStep {
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
}

pub enum ShaderKind {
    Vertex = 0,
    Fragment = 1,
}

#[derive(Clone)]
pub struct Shader {
    pub shader_module: Arc<ShaderModule>,
    pub texture_bindings: Vec<TextureBinding>,
    pub global_uniform_bindings: Vec<GlobalUniformBinding>,
    pub local_uniform_bindings: Vec<LocalUniformBinding>,
}

#[derive(Clone)]
pub struct TextureBinding {
    pub texture: Arc<Texture>,
    pub descriptor_set_binding: u32,
}

#[derive(Clone)]
pub struct LocalUniformBinding {
    pub control: Rc<Control>,
    pub component_count: u32,
    pub uniform_buffer_offset: u32,
}

#[derive(Clone)]
pub struct GlobalUniformBinding {
    pub global_type: GlobalType,
    pub uniform_buffer_f32_offset: u32,
}
