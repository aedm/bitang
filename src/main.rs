mod file;
mod render;
mod tool;
mod types;

use crate::file::blend_loader::load_blend_file;
use crate::render::vulkan_window::VulkanWindow;
use crate::tool::demo_tool::DemoTool;
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    let object = load_blend_file("app/naty/File.blend")?;
    let window = VulkanWindow::new();
    let app = DemoTool::new(&window.context, &window.event_loop, object);
    window.run(app);
    Ok(())
}
