// use crate::render::VulkanRenderer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

pub type Vertex = ([f32; 3], [f32; 3], [f32; 2]);
pub type Face = [Vertex; 3];

#[derive(Debug)]
pub struct Mesh {
    pub faces: Vec<Face>,
}

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub location: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub mesh: Mesh,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VertexV3 {
    pub pos: [f32; 4],
    pub color: [f32; 4],
    pub tex_coord: [f32; 2],
}
