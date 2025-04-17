use std::sync::Arc;

use anyhow::{Context, Result};

use super::image::BitangImage;
use super::Size2D;
use crate::engine::Globals;

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
    pub screen_viewport: Viewport,
    pub command_encoder: wgpu::CommandEncoder,
    pub globals: Globals,
}

pub struct RenderPassContext<'pass> {
    pub gpu_context: &'pass GpuContext,
    pub pass: wgpu::RenderPass<'pass>,
    pub globals: &'pass mut Globals,
}

pub struct ComputePassContext<'pass> {
    pub gpu_context: &'pass GpuContext,
    pub pass: wgpu::ComputePass<'pass>,
    pub globals: &'pass mut Globals,
}
