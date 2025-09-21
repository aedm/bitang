use std::sync::Arc;

use anyhow::{Context, Result};
use smallvec::SmallVec;

use super::globals::Globals;
use super::image::BitangImage;
use super::Size2D;

pub struct GpuContext {
    #[allow(dead_code)]
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,

    // TODO: rename to swapchain something
    // TODO: remove from here
    pub final_render_target: Arc<BitangImage>,
}

impl GpuContext {
    pub async fn new_for_offscreen(final_render_target: Arc<BitangImage>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .context("No suitable adapter found")?;
        let device_descriptor = wgpu::DeviceDescriptor {
            required_features: wgpu::Features::FLOAT32_FILTERABLE
                | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                | wgpu::Features::VERTEX_WRITABLE_STORAGE,
            ..Default::default()
        };
        let (device, queue) = adapter.request_device(&device_descriptor, None).await?;

        Ok(GpuContext {
            adapter,
            queue,
            device,
            final_render_target,
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub size: Size2D,
}

pub struct FrameContext {
    // TODO: remove Arc
    pub gpu_context: Arc<GpuContext>,
    pub screen_size: Size2D,
    pub command_encoder: wgpu::CommandEncoder,
    pub globals: Globals,
    pub screen_pass_draw_batch: RenderPassDrawBatch,
}


// A draw command that belong to a certain render pass
pub struct RenderPassDrawCommand {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: Option<wgpu::Buffer>,
    pub vertex_bind_group: wgpu::BindGroup,
    pub fragment_bind_group: wgpu::BindGroup,
    pub vertex_count: u32,
    pub index_count: u32,
    pub instance_count: u32,
}

// All data needed to render a render pass.
// This struct is Sync so it can be passed to egui's paint callback and render to screen.
#[derive(Default)]
pub struct RenderPassDrawBatch {
    pub draw_commands: SmallVec<[RenderPassDrawCommand; 32]>,
}

impl RenderPassDrawBatch {
    pub fn render(
        &self,
        render_pass: &mut wgpu::RenderPass,
    ) {
        for draw_command in &self.draw_commands {
            render_pass.set_pipeline(&draw_command.pipeline);
            render_pass.set_vertex_buffer(0, draw_command.vertex_buffer.slice(..));
            render_pass.set_bind_group(0, &draw_command.vertex_bind_group, &[]);
            render_pass.set_bind_group(1, &draw_command.fragment_bind_group, &[]);
            if let Some(index_buffer) = &draw_command.index_buffer {
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..draw_command.index_count,
                    0,
                    0..draw_command.instance_count,
                );
            } else {
                render_pass.draw(0..draw_command.vertex_count, 0..draw_command.instance_count);
            }
        }
    }

    // fn make_color_attachment<'a>(
    //     texture_view: &'a wgpu::TextureView,
    //     clear_color: &Option<[f32; 4]>,
    // ) -> wgpu::RenderPassColorAttachment<'a> {
    //     let load = match clear_color {
    //         Some(clear_color) => wgpu::LoadOp::Clear(wgpu::Color {
    //             r: clear_color[0] as f64,
    //             g: clear_color[1] as f64,
    //             b: clear_color[2] as f64,
    //             a: clear_color[3] as f64,
    //         }),
    //         None => wgpu::LoadOp::Load,
    //     };

    //     wgpu::RenderPassColorAttachment {
    //         view: texture_view,
    //         resolve_target: None,
    //         ops: wgpu::Operations {
    //             load,
    //             store: wgpu::StoreOp::Store,
    //         },
    //     }
    // }

    // fn make_depth_attachment<'a>(
    //     texture_view: &'a wgpu::TextureView,
    //     should_clear: bool,
    // ) -> wgpu::RenderPassDepthStencilAttachment<'a> {
    //     let load = if should_clear { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load };

    //     wgpu::RenderPassDepthStencilAttachment {
    //         view: texture_view,
    //         depth_ops: Some(wgpu::Operations {
    //             load,
    //             store: wgpu::StoreOp::Store,
    //         }),
    //         stencil_ops: None,
    //     }
    // }
}

pub struct RenderPassContext<'pass> {
    pub gpu_context: &'pass GpuContext,
    // pub pass: wgpu::RenderPass<'pass>,
    pub globals: &'pass mut Globals,
    pub pass_queue: &'pass mut RenderPassDrawBatch,
}

pub struct ComputePassContext<'pass> {
    pub gpu_context: &'pass GpuContext,
    pub pass: wgpu::ComputePass<'pass>,
    pub globals: &'pass mut Globals,
}
