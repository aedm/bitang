use super::shader::Shader;
use crate::tool::{ComputePassContext, GpuContext};
use anyhow::Result;

/// Represents a compute shader with all necessary resources.
pub struct ComputeCall {
    pub id: String,
    shader: Shader,
    pipeline: wgpu::ComputePipeline,
    dispatch_count: u32,
}

impl ComputeCall {
    /// Creates a new compute shader call.
    /// 
    /// `invocation_count` - The number of invocations. The compute shader will run _AT_LEAST_ this
    /// many times. Note: the actual number of invocations may be higher since it's a multiple of the workgroup size.
    /// It's the responsibility of the actual compute shader code to handle this. For example: if you want to
    /// calculate something for each item in a buffer which has 100 items, then `invocation_count` should be 100.
    /// If the the workgroup size is 64, the compute shader will run 128 times since that's the smallest multiple of 64
    /// that is greater than or equal to 100.
    pub fn new(context: &GpuContext, id: &str, shader: Shader, invocation_count: usize) -> Result<ComputeCall> {
        let pipeline_layout =
            context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&shader.bind_group_layout],
                push_constant_ranges: &[],
            });
        let entry_point = shader.kind.entry_point();
        let pipeline = context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shader.shader_module,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // TODO: use the actual workgroup size
        const WORKGROUP_SIZE: usize = 64;
        let dispatch_count = ((invocation_count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE).try_into()?;
        Ok(ComputeCall {
            id: id.to_string(),
            shader,
            pipeline,
            dispatch_count,
        })
    }

    pub fn execute(&self, context: &mut ComputePassContext<'_>) -> Result<()> {
        context.pass.set_pipeline(&self.pipeline);
        self.shader.bind_to_compute_pass(context)?;
        context.pass.dispatch_workgroups(self.dispatch_count, 1, 1);
        Ok(())
    }
}
