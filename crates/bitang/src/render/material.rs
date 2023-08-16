use crate::render::mesh::Mesh;
use crate::render::shader::{Shader, ShaderKind};
use crate::render::vulkan_window::{RenderContext, VulkanContext};
use crate::render::Vertex3;
use anyhow::{Context, Result};
use serde::Deserialize;
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

#[derive(Debug, Deserialize, Default, Clone)]
pub enum BlendMode {
    #[default]
    None,
    Alpha,
    Additive,
}

pub struct Material {
    pub passes: Vec<Option<MaterialPass>>,
}

impl Material {
    pub fn get_pass(&self, pass_id: usize) -> Option<&MaterialPass> {
        self.passes[pass_id].as_ref()
    }
}

pub struct MaterialPass {
    pub id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_mode: BlendMode,

    // Vulkan stuff
    pipeline: Arc<GraphicsPipeline>,
    vertex_layout: Option<Arc<DescriptorSetLayout>>,
    fragment_layout: Option<Arc<DescriptorSetLayout>>,
}

impl MaterialPass {
    pub fn new(
        context: &VulkanContext,
        id: String,
        vertex_shader: Shader,
        fragment_shader: Shader,
        depth_test: bool,
        depth_write: bool,
        blend_mode: BlendMode,
        vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<MaterialPass> {
        let depth_state = if depth_test || depth_write {
            let compare_op = if depth_test { CompareOp::Less } else { CompareOp::Always };
            Some(DepthState {
                enable_dynamic: false,
                compare_op: StateMode::Fixed(compare_op),
                write_enable: StateMode::Fixed(depth_write),
            })
        } else {
            None
        };

        let depth_stencil_state = DepthStencilState {
            depth: depth_state,
            depth_bounds: Default::default(),
            stencil: Default::default(),
        };

        let mut color_blend_state = ColorBlendState::new(1);
        match blend_mode {
            BlendMode::None => {}
            BlendMode::Alpha => {
                color_blend_state = color_blend_state.blend_alpha();
            }
            BlendMode::Additive => {
                color_blend_state = color_blend_state.blend(AttachmentBlend {
                    color_op: BlendOp::Add,
                    color_source: BlendFactor::SrcAlpha,
                    color_destination: BlendFactor::One,
                    alpha_op: BlendOp::Max,
                    alpha_source: BlendFactor::One,
                    alpha_destination: BlendFactor::One,
                });
            }
        };

        // Create the Vulkan pipeline
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(Vertex3::per_vertex())
            .vertex_shader(
                vertex_shader
                    .shader_module
                    .entry_point("main")
                    .context("Failed to get vertex shader entry point")?,
                (),
            )
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(
                fragment_shader
                    .shader_module
                    .entry_point("main")
                    .context("Failed to get fragment shader entry point")?,
                (),
            )
            .color_blend_state(color_blend_state)
            .depth_stencil_state(depth_stencil_state)
            // Unwrap is safe: every pass has one subpass
            .render_pass(Subpass::from(vulkan_render_pass, 0).unwrap())
            .build(context.context.device().clone())?;

        // Extract the descriptor set layouts
        let descriptor_set_layouts = pipeline.layout().set_layouts();
        let vertex_layout = descriptor_set_layouts
            .get(ShaderKind::Vertex.get_descriptor_set_index() as usize)
            .cloned();
        let fragment_layout = descriptor_set_layouts
            .get(ShaderKind::Fragment.get_descriptor_set_index() as usize)
            .cloned();

        Ok(MaterialPass {
            id,
            vertex_shader,
            fragment_shader,
            depth_test,
            depth_write,
            blend_mode,
            pipeline,
            vertex_layout,
            fragment_layout,
        })
    }

    pub fn render(&self, context: &mut RenderContext, mesh: &Mesh) -> Result<()> {
        let pipeline_layout = self.pipeline.layout();
        context
            .command_builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, mesh.vertex_buffer.clone());
        self.vertex_shader.bind(context, pipeline_layout)?;
        self.fragment_shader.bind(context, pipeline_layout)?;

        let instance_count = context.globals.instance_count as u32;
        context
            .command_builder
            .draw(mesh.vertex_buffer.len() as u32, instance_count, 0, 0)?;
        Ok(())
    }
}
