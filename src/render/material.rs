use crate::render::shader::Shader;
use crate::render::vulkan_window::VulkanContext;
use crate::render::Vertex3;
use std::sync::Arc;
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::{GraphicsPipeline, StateMode};

pub enum MaterialStepType {
    EarlyDepth = 0,
    Shadow = 1,
    Solid = 2,
}
pub const MATERIAL_STEP_COUNT: usize = 3;

pub struct Material {
    pub passes: [Option<MaterialStep>; MATERIAL_STEP_COUNT],
}

pub struct MaterialStep {
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
}
