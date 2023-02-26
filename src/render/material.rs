use crate::control::controls::{Control, ControlValue};
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
    pub uniform_bindings: Vec<UniformBinding>,
}

#[derive(Clone)]
pub struct TextureBinding {
    pub texture: Arc<Texture>,
    pub descriptor_set_binding: u32,
}

pub struct UniformBinding {
    pub control: Rc<Control>,
    pub component_count: u32,
    pub uniform_buffer_offset: u32,
}
