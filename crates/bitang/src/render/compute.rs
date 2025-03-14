use crate::render::buffer::Buffer;
use crate::render::shader::Shader;
use crate::tool::{ComputePassContext, GpuContext};
use anyhow::Result;
use std::rc::Rc;

pub enum Run {
    Init(Rc<Buffer>),
    Simulate(Rc<Buffer>),
}

/// Represents a compute step in the chart sequence.
pub struct Compute {
    pub id: String,
    pub shader: Shader,
    pub run: Run,
    pipeline: wgpu::ComputePipeline,
}

impl Compute {
    pub fn new(
        context: &GpuContext,
        id: &str,
        shader: Shader,
        run: Run,
    ) -> Result<Compute> {
        let pipeline_layout =
        context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &shader.bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        // TODO: handle different entry points for init and simulate
        let entry_point = match run {
            Run::Init(_) => "cs_main",
            Run::Simulate(_) => "cs_main",
        };
        let pipeline = context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shader.shader_module,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(Compute {
            id: id.to_string(),
            shader,
            run,
            pipeline,
        })
    }

    pub fn execute(&self, context: &mut ComputePassContext<'_>) -> Result<()> {
        // TODO: document why 64
        let dispatch_count = match &self.run {
            Run::Init(buffer) => buffer.item_size_in_vec4 * buffer.item_count / 64,
            Run::Simulate(buffer) => {
                buffer.step();
                (buffer.item_size_in_vec4 * buffer.item_count + 63) / 64
            }
        };

        context.pass.set_pipeline(&self.pipeline);
        self.shader.bind_to_compute_pass(context)?;
        context.pass.dispatch_workgroups(dispatch_count as u32, 1, 1);

        Ok(())
    }
}
