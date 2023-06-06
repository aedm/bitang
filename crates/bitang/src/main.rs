mod control;
mod file;
mod render;
mod tool;

use crate::render::vulkan_window::VulkanWindow;
use crate::tool::demo_tool::DemoTool;
use anyhow::Result;
use build_time::{build_time_local, build_time_utc};
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn set_up_tracing() -> Result<()> {
    #[cfg(windows)]
    let with_color = nu_ansi_term::enable_ansi_support().is_ok();
    #[cfg(not(windows))]
    let with_color = true;

    let fmt_layer = fmt::layer().with_target(false).with_ansi(with_color);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(if cfg!(debug_assertions) { "debug" } else { "info" }))?;
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
    Ok(())
}

fn main() -> Result<()> {
    set_up_tracing()?;
    if VERSION == "0.0.0" {
        info!("Bitang dev version, build time {}", build_time_local!());
    } else {
        info!("Bitang {VERSION}");
    }

    let window = VulkanWindow::new()?;
    let app = DemoTool::new(&window.context, window.event_loop.as_ref().unwrap())?;
    window.run(app);
    Ok(())
}
