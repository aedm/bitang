use crate::render::VulkanRenderer;
use vulkano::command_buffer::AutoCommandBufferBuilder;

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

// impl VertexV3 {
//     pub fn get_binding_descriptions() -> [vk::VertexInputBindingDescription; 1] {
//         [vk::VertexInputBindingDescription {
//             binding: 0,
//             stride: std::mem::size_of::<Self>() as u32,
//             input_rate: vk::VertexInputRate::VERTEX,
//         }]
//     }
//
//     pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
//         [
//             vk::VertexInputAttributeDescription {
//                 binding: 0,
//                 location: 0,
//                 format: vk::Format::R32G32B32A32_SFLOAT,
//                 offset: offset_of!(Self, pos) as u32,
//             },
//             vk::VertexInputAttributeDescription {
//                 binding: 0,
//                 location: 1,
//                 format: vk::Format::R32G32B32A32_SFLOAT,
//                 offset: offset_of!(Self, color) as u32,
//             },
//             vk::VertexInputAttributeDescription {
//                 binding: 0,
//                 location: 2,
//                 format: vk::Format::R32G32_SFLOAT,
//                 offset: offset_of!(Self, tex_coord) as u32,
//             },
//         ]
//     }
// }

pub trait App {
    fn draw(&mut self, renderer: &VulkanRenderer, builder: AutoCommandBufferBuilder<i32>);
}
