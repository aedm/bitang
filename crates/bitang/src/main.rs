mod control;
mod file;
mod render;
mod tool;

use crate::render::vulkan_window::VulkanWindow;
use crate::tool::demo_tool::DemoTool;
use anyhow::Result;
use tracing::{debug, info};

fn main() -> Result<()> {
    // Set up tracing
    tracing_subscriber::fmt::init();
    debug!("Starting up DEBUG");
    info!("Starting up INFO");

    let window = VulkanWindow::new();
    let app = DemoTool::new(&window.context, &window.event_loop)?;
    window.run(app);
    Ok(())
}
