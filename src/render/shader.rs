use crate::render::Texture;
use std::sync::Arc;
use vulkano::shader::ShaderModule;

pub enum ShaderKind {
    Vertex = 0,
    Fragment = 1,
}

pub struct Shader {
    pub shader_module: Arc<ShaderModule>,
    pub textures: Vec<Arc<Texture>>,
}
