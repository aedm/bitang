use crate::render::mesh::Mesh;
use crate::render::shader::Shader;
use crate::render::vulkan_window::RenderContext;
use anyhow::{Context, Result};
use std::sync::Arc;
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferUsage;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend, BlendFactor, BlendOp, ColorBlendState,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::pipeline::{Pipeline, StateMode};
use vulkano::render_pass::Subpass;
use vulkano::sampler::{Sampler, SamplerAddressMode, SamplerCreateInfo};

#[derive(Clone)]
pub enum BlendMode {
    None,
    Alpha,
    Additive,
}

#[derive(Clone)]
pub struct Material {
    pub passes: Vec<Option<MaterialPass>>,
}

impl Material {
    pub fn get_pass(&self, pass_id: usize) -> Option<&MaterialPass> {
        self.passes[pass_id].as_ref()
    }
}

#[derive(Clone)]
pub struct MaterialPass {
    pub id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_mode: BlendMode,

    pipeline: Arc<GraphicsPipeline>,
}

impl MaterialPass {
    pub fn render(&self, context: &mut RenderContext, mesh: &Mesh) -> Result<()> {
        // let descriptor_set_layouts = self.pipeline.layout().set_layouts();

        context
            .command_builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, mesh.vertex_buffer.clone());
        self.vertex_shader
            .bind(context, self.pipeline.layout().clone())?;
        self.fragment_shader
            .bind(context, self.pipeline.layout().clone())?;

        let instance_count = context.globals.instance_count as u32;
        context
            .command_builder
            .draw(mesh.vertex_buffer.len() as u32, instance_count, 0, 0)?;
        Ok(())
    }
}
