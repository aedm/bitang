use std::sync::Arc;

use anyhow::{Context, Result};

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

pub enum RenderStage<'frame> {
    Offscreen(&'frame mut wgpu::CommandEncoder),
    Onscreen(&'frame mut wgpu::RenderPass<'static>),
}

pub struct FrameContext<'frame> {
    // TODO: remove Arc
    pub gpu_context: Arc<GpuContext>,
    pub screen_viewport: Viewport,
    // pub command_encoder: &'frame mut wgpu::CommandEncoder,
    pub globals: Globals,

    /// Content is rendered in two steps: offscreen and onscreen rendering.
    /// If the screen renderpass is available, only the onscreen rendering is done.
    /// Otherwise, the offscreen rendering is done.
    pub render_stage: RenderStage<'frame>,
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
