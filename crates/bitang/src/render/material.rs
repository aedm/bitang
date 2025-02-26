use crate::render::mesh::Mesh;
use crate::render::shader::Shader;
use crate::render::Vertex3;
use crate::tool::{FrameContext, GpuContext, RenderPassContext};
use anyhow::{Context, Result};
use serde::Deserialize;
use smallvec::SmallVec;
use std::sync::Arc;
use wgpu::CompareFunction;

use super::pass::{FramebufferInfo, Pass};
use super::VERTEX_FORMAT;
// use vulkano::pipeline::graphics::color_blend::{
//     AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
// };
// use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
// use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
// use vulkano::pipeline::graphics::rasterization::RasterizationState;
// use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
// use vulkano::pipeline::graphics::viewport::ViewportState;
// use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
// use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
// use vulkano::pipeline::{DynamicState, Pipeline, PipelineLayout};
// use vulkano::pipeline::{GraphicsPipeline, PipelineShaderStageCreateInfo};
// use vulkano::render_pass::Subpass;

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
    pub _id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,

    // TODO: check why these are here
    pub _depth_test: bool,
    pub _depth_write: bool,
    pub _blend_mode: BlendMode,
    pipeline: wgpu::RenderPipeline,
    // pipeline: Arc<GraphicsPipeline>,
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
        context: &GpuContext,
        props: MaterialPassProps,
        framebuffer_info: &FramebufferInfo,
        // vulkan_render_pass: Arc<vulkano::render_pass::RenderPass>,
    ) -> Result<MaterialPass> {
        // let uniform_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
        //     label: Some("Uniform Buffer"),
        //     size: size_of::<Uniforms>() as u64,
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        //     mapped_at_creation: false,
        // });

        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[
                        &props.vertex_shader.bind_group_layout,
                        &props.fragment_shader.bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex3>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_FORMAT,
        };

        let blend_state = match props.blend_mode {
            BlendMode::None => wgpu::BlendState::REPLACE,
            BlendMode::Alpha => wgpu::BlendState::ALPHA_BLENDING,
            BlendMode::Additive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Max,
                },
            },
        };

        let mut fragment_targets = SmallVec::<[_; 8]>::new();
        for color_buffer_format in &framebuffer_info.color_buffer_formats {
            fragment_targets.push(Some(wgpu::ColorTargetState {
                format: color_buffer_format.wgpu_format(),
                blend: Some(blend_state),
                write_mask: wgpu::ColorWrites::ALL,
            }));
        }

        let depth_stencil = framebuffer_info.depth_buffer_format.map(|format| {
            let depth_compare =
                if props.depth_test { CompareFunction::LessEqual } else { CompareFunction::Always };
            wgpu::DepthStencilState {
                format: format.wgpu_format(),
                depth_write_enabled: props.depth_write,
                depth_compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }
        });

        let pipeline = context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &props.vertex_shader.shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_buffer_layout],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &props.fragment_shader.shader_module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &fragment_targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Ok(MaterialPass {
            _id: props.id,
            vertex_shader: props.vertex_shader,
            fragment_shader: props.fragment_shader,
            _depth_test: props.depth_test,
            _depth_write: props.depth_write,
            _blend_mode: props.blend_mode,
            pipeline,
        })
        // Unwrap is safe: every pass has exactly one subpass
        // let subpass = Subpass::from(vulkan_render_pass, 0).unwrap();

        // let depth_stencil_state = if subpass.subpass_desc().depth_stencil_attachment.is_none() {
        //     None
        // } else {
        //     let compare_op =
        //         if props.depth_test { CompareOp::LessOrEqual } else { CompareOp::Always };
        //     Some(DepthStencilState {
        //         depth: Some(DepthState {
        //             compare_op,
        //             write_enable: props.depth_write,
        //         }),
        //         ..Default::default()
        //     })
        // };

        // let color_blend_state = if subpass.num_color_attachments() == 0 {
        //     None
        // } else {
        //     let blend_state = match props.blend_mode {
        //         BlendMode::None => AttachmentBlend {
        //             color_blend_op: BlendOp::Add,
        //             src_color_blend_factor: BlendFactor::SrcAlpha,
        //             dst_color_blend_factor: BlendFactor::Zero,
        //             alpha_blend_op: BlendOp::Max,
        //             src_alpha_blend_factor: BlendFactor::One,
        //             dst_alpha_blend_factor: BlendFactor::One,
        //         },
        //         BlendMode::Alpha => AttachmentBlend::alpha(),
        //         BlendMode::Additive => AttachmentBlend {
        //             color_blend_op: BlendOp::Add,
        //             src_color_blend_factor: BlendFactor::SrcAlpha,
        //             dst_color_blend_factor: BlendFactor::One,
        //             alpha_blend_op: BlendOp::Max,
        //             src_alpha_blend_factor: BlendFactor::One,
        //             dst_alpha_blend_factor: BlendFactor::One,
        //         },
        //     };
        //     Some(ColorBlendState::with_attachment_states(
        //         subpass.num_color_attachments(),
        //         ColorBlendAttachmentState {
        //             blend: Some(blend_state),
        //             ..Default::default()
        //         },
        //     ))
        // };

        // Create the Vulkan pipeline
        // let pipeline = {
        //     // TODO: store the entry point instead of the module
        //     let vs = props
        //         .vertex_shader
        //         .shader_module
        //         .entry_point("vs_main")
        //         .unwrap();
        //     let fs = props
        //         .fragment_shader
        //         .shader_module
        //         .entry_point("fs_main")
        //         .unwrap();

        //     let vertex_input_state =
        //         Some(Vertex3::per_vertex().definition(&vs.info().input_interface)?);
        //     let stages = [
        //         PipelineShaderStageCreateInfo::new(vs),
        //         PipelineShaderStageCreateInfo::new(fs),
        //     ];
        //     let layout = PipelineLayout::new(
        //         context.device.clone(),
        //         PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
        //             .into_pipeline_layout_create_info(context.device.clone())?,
        //     )?;
        //     let pipeline_creation_info = GraphicsPipelineCreateInfo {
        //         input_assembly_state: Some(InputAssemblyState::default()),
        //         vertex_input_state,
        //         stages: stages.into_iter().collect(),
        //         dynamic_state: [DynamicState::Viewport].into_iter().collect(),
        //         color_blend_state,
        //         depth_stencil_state,
        //         viewport_state: Some(ViewportState::default()),
        //         multisample_state: Some(Default::default()),
        //         rasterization_state: Some(RasterizationState::default()),
        //         subpass: Some(subpass.into()),
        //         ..GraphicsPipelineCreateInfo::layout(layout)
        //     };
        //     GraphicsPipeline::new(context.device.clone(), None, pipeline_creation_info)
        //         .with_context(|| {
        //             format!(
        //                 "Failed to create graphics pipeline for material pass: {}",
        //                 props.id,
        //             )
        //         })?
        // };
    }

    pub fn render(&self, context: &mut RenderPassContext, mesh: &Mesh) -> Result<()> {
        context.pass.set_pipeline(&self.pipeline);
        context
            .pass
            .set_vertex_buffer(0, mesh.vertex_buffer.slice(..));

        // let pipeline_layout = self.pipeline.layout();
        let instance_count = context.globals.instance_count as u32;
        // context
        //     .command_builder
        //     .bind_pipeline_graphics(self.pipeline.clone())?
        //     .bind_vertex_buffers(0, mesh.vertex_buffer.clone())?;
        self.vertex_shader.bind(context)?;
        self.fragment_shader.bind(context)?;

        match &mesh.index_buffer {
            None => {
                context.pass.draw(0..mesh.vertex_count, 0..instance_count);
                // context.command_builder.draw(
                //     mesh.vertex_buffer.len() as u32,
                //     instance_count,
                //     0,
                //     0,
                // )?;
            }
            Some(index_buffer) => {
                context
                    .pass
                    .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                context
                    .pass
                    .draw_indexed(0..mesh.index_count, 0, 0..instance_count);
                // context
                //     .command_builder
                //     .bind_index_buffer(index_buffer.clone())?
                //     .draw_indexed(index_buffer.len() as u32, instance_count, 0, 0, 0)?;
            }
        }
        Ok(())
    }
}
