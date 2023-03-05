mod control;
mod file;
mod render;
mod tool;

use crate::render::vulkan_window::VulkanWindow;
use crate::tool::demo_tool::DemoTool;
use anyhow::Result;

fn main() -> Result<()> {
    let window = VulkanWindow::new();
    let app = DemoTool::new(&window.context, &window.event_loop)?;
    window.run(app);
    Ok(())
}
