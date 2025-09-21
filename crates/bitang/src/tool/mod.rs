mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui;

use anyhow::Result;
// use runners::frame_dump_runner::FrameDumpRunner;
use runners::window_runner::WindowRunner;

use crate::engine::PixelFormat;

pub const FRAMEDUMP_MODE: bool = false;
pub const FRAMEDUMP_WIDTH: u32 = 3840;
pub const FRAMEDUMP_HEIGHT: u32 = 2160;
pub const FRAMEDUMP_FPS: u32 = 60;

const SCREEN_RATIO: (u32, u32) = (16, 9);

pub const FRAMEDUMP_PIXEL_FORMAT: PixelFormat = PixelFormat::Rgba8U;

pub fn run_app() -> Result<()> {
    if FRAMEDUMP_MODE {
        // FrameDumpRunner::run()
        unimplemented!()
    } else {
        WindowRunner::run()
    }
}
