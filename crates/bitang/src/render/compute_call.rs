// use crate::render::shader::Shader;
// use crate::tool::{ComputePassContext, GpuContext};
// use anyhow::Result;

// /// A compute call to the wgpu API
// pub struct ComputeCall {
//     pub id: String,
//     shader: Shader,
//     pipeline: wgpu::ComputePipeline,
//     dispatch_count: u32,
// }

// impl ComputeCall {
//     pub fn new(context: &GpuContext, id: &str, shader: Shader, dispatch_count: u32) -> Result<ComputeCall> {
//         let pipeline_layout =
//             context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//                 label: None,
//                 bind_group_layouts: &[&shader.bind_group_layout],
//                 push_constant_ranges: &[],
//             });
//         let pipeline = context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
//             label: None,
//             layout: Some(&pipeline_layout),
//             module: &shader.shader_module,
//             entry_point: Some(shader.kind.entry_point()),
//             compilation_options: wgpu::PipelineCompilationOptions::default(),
//             cache: None,
//         });

//         Ok(ComputeCall {
//             id: id.to_string(),
//             shader,
//             pipeline,
//             dispatch_count,
//         })
//     }

//     pub fn execute(&self, context: &mut ComputePassContext<'_>) -> Result<()> {
//         context.pass.set_pipeline(&self.pipeline);
//         self.shader.bind_to_compute_pass(context)?;
//         context.pass.dispatch_workgroups(self.dispatch_count, 1, 1);
//         Ok(())
//     }
// }
