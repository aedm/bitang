use crate::engine::pass::FramebufferInfo;
use super::mesh::Mesh;
use super::shader::Shader;
use super::Vertex3;
use super::context::{GpuContext, RenderPassContext};
use anyhow::Result;
use serde::Deserialize;
use smallvec::SmallVec;
use wgpu::CompareFunction;

use super::VERTEX_FORMAT;

#[derive(Debug, Deserialize, Default, Clone)]
pub enum BlendMode {
    #[default]
    None,
    Alpha,
    Additive,
}


pub struct DrawCall {
    pub _id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pipeline: wgpu::RenderPipeline,
}

pub struct DrawCallProps {
    pub id: String,
    pub vertex_shader: Shader,
    pub fragment_shader: Shader,
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend_mode: BlendMode,
}

impl DrawCall {
    pub fn new(
        context: &GpuContext,
        props: DrawCallProps,
        framebuffer_info: &FramebufferInfo,
    ) -> Result<DrawCall> {
        let pipeline_layout =
            context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        let pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

        Ok(DrawCall {
            _id: props.id,
            vertex_shader: props.vertex_shader,
            fragment_shader: props.fragment_shader,
            pipeline,
        })
    }

    pub fn render(&self, context: &mut RenderPassContext, mesh: &Mesh) -> Result<()> {
        context.pass.set_pipeline(&self.pipeline);
        context.pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));

        let instance_count = context.globals.instance_count as u32;
        self.vertex_shader.bind_to_render_pass(context)?;
        self.fragment_shader.bind_to_render_pass(context)?;

        match &mesh.index_buffer {
            None => {
                context.pass.draw(0..mesh.vertex_count, 0..instance_count);
            }
            Some(index_buffer) => {
                context.pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                context.pass.draw_indexed(0..mesh.index_count, 0, 0..instance_count);
            }
        }
        Ok(())
    }
}
