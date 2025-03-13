mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui;

use crate::control::controls::Globals;
use crate::render::image::{BitangImage, PixelFormat};
use crate::render::{Size2D, SCREEN_COLOR_FORMAT, SCREEN_RENDER_TARGET_ID};
// use crate::tool::runners::frame_dump_runner::FrameDumpRunner;
use anyhow::{Context, Result};
use runners::window_runner::WindowRunner;
use std::default::Default;
use std::sync::Arc;

const START_IN_DEMO_MODE: bool = false;
const BORDERLESS_FULL_SCREEN: bool = true;

pub const FRAMEDUMP_MODE: bool = false;
pub const FRAMEDUMP_WIDTH: u32 = 3840;
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
pub const FRAMEDUMP_FPS: u32 = 61;

const SCREEN_RATIO: (u32, u32) = (16, 9);

pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,

    // TODO: rename to swapchain something
    pub final_render_target: Arc<BitangImage>,
}

impl GpuContext {
    fn new(swapchain_pixel_format: PixelFormat) -> Result<Arc<Self>> {
        tokio::runtime::Runtime::new()?.block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .context("No suitable adapter found")?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::FLOAT32_FILTERABLE,
                    ..wgpu::DeviceDescriptor::default()
                }, None)
                .await?;

            let final_render_target =
                BitangImage::new_swapchain(SCREEN_RENDER_TARGET_ID, swapchain_pixel_format);

            Ok(Arc::new(Self {
                adapter,
                queue,
                device,
                final_render_target,
            }))
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
    pub gpu_context: Arc<GpuContext>,
    pub screen_viewport: Viewport,
    pub canvas_size: Size2D,
    pub command_encoder: wgpu::CommandEncoder,
    pub globals: Globals,
    pub simulation_elapsed_time_since_last_render: f32,
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
    pub simulation_elapsed_time_since_last_render: &'pass mut f32,
}

pub fn run_app() -> Result<()> {
    if FRAMEDUMP_MODE {
        todo!()
        // FrameDumpRunner::run()
    } else {
        WindowRunner::run()
    }
}
