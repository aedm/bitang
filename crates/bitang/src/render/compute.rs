use crate::render::buffer::Buffer;
use crate::render::shader::Shader;
use crate::tool::{RenderContext, VulkanContext};
use anyhow::{Context, Result};
use std::rc::Rc;
use std::sync::Arc;
use vulkano::pipeline::compute::ComputePipelineCreateInfo;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{ComputePipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo};

pub enum Run {
    Init(Rc<Buffer>),
    Simulate(Rc<Buffer>),
}

/// Represents a compute step in the chart sequence.
pub struct Compute {
    pub id: String,
    pub shader: Shader,
    pub run: Run,
    pipeline: Arc<ComputePipeline>,
}

impl Compute {
    pub fn new(
        context: &Arc<VulkanContext>,
        id: &str,
        shader: Shader,
        run: Run,
    ) -> Result<Compute> {
        let cs = shader
            .shader_module
            .entry_point("main")
            .context("Failed to get compute shader entry point")?;
        let stage = PipelineShaderStageCreateInfo::new(cs);
        let layout_create_info = PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage])
            .into_pipeline_layout_create_info(context.device.clone())?;
        let layout = PipelineLayout::new(context.device.clone(), layout_create_info)?;
        let pipeline = ComputePipeline::new(
            context.device.clone(),
            None,
            ComputePipelineCreateInfo::stage_layout(stage, layout),
        )?;

        Ok(Compute {
            id: id.to_string(),
            shader,
            run,
            pipeline,
        })
    }

    pub fn execute(&self, context: &mut RenderContext) -> Result<()> {
        let dispatch_count = match &self.run {
            Run::Init(buffer) => buffer.item_size_in_vec4 * buffer.item_count / 64,
            Run::Simulate(buffer) => {
                buffer.step();
                (buffer.item_size_in_vec4 * buffer.item_count + 63) / 64
            }
        };
        context
            .command_builder
            .bind_pipeline_compute(self.pipeline.clone())?;
        self.shader.bind(context, self.pipeline.layout())?;
        context
            .command_builder
            .dispatch([dispatch_count as u32, 1, 1])?;

        Ok(())
    }
}
