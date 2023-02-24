use crate::render::shader::Shader;

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
