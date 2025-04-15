mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui; 

use crate::control::controls::Globals;
use crate::engine::{BitangImage, ImageSizeRule};
use crate::engine::{Size2D, FRAMEDUMP_PIXEL_FORMAT, SCREEN_RENDER_TARGET_ID};
use anyhow::{Context, Result};
use runners::frame_dump_runner::FrameDumpRunner;
use runners::window_runner::WindowRunner;
use std::default::Default;
use std::sync::Arc;

const START_IN_DEMO_MODE: bool = false;

pub const FRAMEDUMP_MODE: bool = false;
pub const FRAMEDUMP_WIDTH: u32 = 3840;
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
pub const FRAMEDUMP_FPS: u32 = 60;

const SCREEN_RATIO: (u32, u32) = (16, 9);

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
    pub async fn new_for_offscreen() -> Result<Self> {
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
            final_render_target: BitangImage::new_attachment(
                SCREEN_RENDER_TARGET_ID,
                FRAMEDUMP_PIXEL_FORMAT,
                ImageSizeRule::Fixed(FRAMEDUMP_WIDTH, FRAMEDUMP_HEIGHT),
                false,
            ),
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

pub fn run_app() -> Result<()> {
    if FRAMEDUMP_MODE {
        FrameDumpRunner::run()
    } else {
        WindowRunner::run()
    }
}
