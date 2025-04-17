mod app_config;
mod app_state;
pub mod content_renderer;
mod music_player;
mod runners;
mod spline_editor;
mod timer;
mod ui; 

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



pub fn run_app() -> Result<()> {
    if FRAMEDUMP_MODE {
        FrameDumpRunner::run()
    } else {
        WindowRunner::run()
    }
}
