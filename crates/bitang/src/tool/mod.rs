mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui;

use crate::control::controls::Globals;
use crate::render::image::BitangImage;
use crate::render::Size2D;
// use crate::tool::runners::frame_dump_runner::FrameDumpRunner;
use anyhow::Result;
use runners::window_runner::WindowRunner;
use std::default::Default;
use std::sync::Arc;

const START_IN_DEMO_MODE: bool = true;

pub const FRAMEDUMP_MODE: bool = false;
#[allow(dead_code)]
pub const FRAMEDUMP_WIDTH: u32 = 3840;
#[allow(dead_code)]
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
#[allow(dead_code)]
pub const FRAMEDUMP_FPS: u32 = 60;

const SCREEN_RATIO: (u32, u32) = (16, 9);

pub struct GpuContext {
    #[allow(dead_code)]
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,

    // TODO: rename to swapchain something
    pub final_render_target: Arc<BitangImage>,
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
        todo!()
        // FrameDumpRunner::run()
    } else {
        WindowRunner::run()
    }
}
