use crate::render::mesh::Mesh;
use crate::render::shader::Shader;
use crate::render::Vertex3;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::sync::Arc;
use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition, VertexInputState};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{DynamicState, Pipeline, PipelineLayout};
use vulkano::pipeline::{GraphicsPipeline, PipelineShaderStageCreateInfo};
use vulkano::render_pass::Subpass;

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
    pipeline: Arc<GraphicsPipeline>,
}

pub struct MaterialPassProps {
    pub id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_mode: BlendMode,
}

impl MaterialPass {
    pub fn new(
        context: &Arc<VulkanContext>,
        props: MaterialPassProps,
        vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<MaterialPass> {
        // let depth_state = if props.depth_test || props.depth_write {
        //     let compare_op = if props.depth_test { CompareOp::Less } else { CompareOp::Always };
        //     Some(DepthState {
        //         compare_op,
        //         write_enable: props.depth_write,
        //     })
        // } else {
        //     None
        // };
        //
        // let depth_stencil_state = DepthStencilState {
        //     depth: depth_state,
        //     ..Default::default()
        // };

        // Unwrap is safe: every pass has exactly one subpass
        let subpass = Subpass::from(vulkan_render_pass, 0).unwrap();

        // let depth_stencil_state = if props.depth_test || props.depth_write {
        //     let compare_op = if props.depth_test { CompareOp::Less } else { CompareOp::Always };
        //     Some(DepthStencilState {
        //         depth: Some(DepthState {
        //             compare_op,
        //             write_enable: props.depth_write,
        //         }),
        //         ..Default::default()
        //     })
        // } else {
        //     None
        // };

        let depth_stencil_state = if subpass.subpass_desc().depth_stencil_attachment.is_none() {
            None
        } else {
            let compare_op = if props.depth_test { CompareOp::Less } else { CompareOp::Always };
            Some(DepthStencilState {
                depth: Some(DepthState {
                    compare_op,
                    write_enable: props.depth_write,
                }),
                ..Default::default()
            })
        };

        let color_blend_state = if subpass.num_color_attachments() == 0 {
            None
        } else {
            let blend_state = match props.blend_mode {
                BlendMode::None => AttachmentBlend::default(),
                BlendMode::Alpha => AttachmentBlend::alpha(),
                BlendMode::Additive => AttachmentBlend {
                    color_blend_op: BlendOp::Add,
                    src_color_blend_factor: BlendFactor::SrcAlpha,
                    dst_color_blend_factor: BlendFactor::One,
                    alpha_blend_op: BlendOp::Max,
                    src_alpha_blend_factor: BlendFactor::One,
                    dst_alpha_blend_factor: BlendFactor::One,
                },
            };
            Some(ColorBlendState::with_attachment_states(
                subpass.num_color_attachments(),
                ColorBlendAttachmentState {
                    blend: Some(blend_state),
                    ..Default::default()
                },
            ))
        };

        // Create the Vulkan pipeline
        let pipeline = {
            // TODO: store the entry point instead of the module
            let vs = props
                .vertex_shader
                .shader_module
                .entry_point("main")
                .unwrap();
            let fs = props
                .fragment_shader
                .shader_module
                .entry_point("main")
                .unwrap();

            let vsmod = vs.info().execution_model;
            let fsmod = fs.info().execution_model;
            println!("Vertex shader execution model: {:?}", vsmod);
            println!("Fragment shader execution model: {:?}", fsmod);

            let vertex_input_state =
                Some(Vertex3::per_vertex().definition(&vs.info().input_interface)?);
            let stages = [
                PipelineShaderStageCreateInfo::new(vs),
                PipelineShaderStageCreateInfo::new(fs),
            ];
            let layout = PipelineLayout::new(
                context.device.clone(),
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                    .into_pipeline_layout_create_info(context.device.clone())?,
            )?;
            let pipeline_creation_info = GraphicsPipelineCreateInfo {
                input_assembly_state: Some(InputAssemblyState::default()),
                vertex_input_state,
                stages: stages.into_iter().collect(),
                dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                color_blend_state,
                depth_stencil_state,
                viewport_state: Some(ViewportState::default()),
                multisample_state: Some(Default::default()),
                rasterization_state: Some(RasterizationState {
                    // rasterizer_discard_enable: true,
                    // depth_clamp_enable: false,
                    // polygon_mode: Default::default(),
                    // cull_mode: Default::default(),
                    // front_face: Default::default(),
                    // depth_bias: None,
                    // line_width: 0.0,
                    // line_rasterization_mode: Default::default(),
                    // line_stipple: None,
                    ..Default::default()
                }),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(layout)
            };
            let x = 1;
            let res = GraphicsPipeline::new(context.device.clone(), None, pipeline_creation_info);
            if let Err(err) = &res {
                let x = format!("error: {:?}", err);
                println!("Error creating gfx pipeline: {:?}", err);
            }
            res?
        };
        // let pipeline = GraphicsPipeline::start()
        //     .vertex_input_state(Vertex3::per_vertex())
        //     .vertex_shader(
        //         props
        //             .vertex_shader
        //             .shader_module
        //             .entry_point("main")
        //             .context("Failed to get vertex shader entry point")?,
        //         (),
        //     )
        //     .input_assembly_state(InputAssemblyState::new())
        //     .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        //     .fragment_shader(
        //         props
        //             .fragment_shader
        //             .shader_module
        //             .entry_point("main")
        //             .context("Failed to get fragment shader entry point")?,
        //         (),
        //     )
        //     .color_blend_state(color_blend_state)
        //     .depth_stencil_state(depth_stencil_state)
        //     // Unwrap is safe: every pass has exactly one subpass
        //     .render_pass(Subpass::from(vulkan_render_pass, 0).unwrap())
        //     .build(context.device.clone())?;

        let y = 1;
        let z = 2;

        Ok(MaterialPass {
            id: props.id,
            vertex_shader: props.vertex_shader,
            fragment_shader: props.fragment_shader,
            depth_test: props.depth_test,
            depth_write: props.depth_write,
            blend_mode: props.blend_mode,
            pipeline,
        })
    }

    pub fn render(&self, context: &mut RenderContext, mesh: &Mesh) -> Result<()> {
        let pipeline_layout = self.pipeline.layout();
        context
            .command_builder
            .bind_pipeline_graphics(self.pipeline.clone())?
            .bind_vertex_buffers(0, mesh.vertex_buffer.clone())?;
        self.vertex_shader.bind(context, pipeline_layout)?;
        self.fragment_shader.bind(context, pipeline_layout)?;

        let instance_count = context.globals.instance_count as u32;
        context
            .command_builder
            .draw(mesh.vertex_buffer.len() as u32, instance_count, 0, 0)?;
        Ok(())
    }
}
