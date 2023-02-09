// mod draw;
mod file;
mod render;
mod tool;
mod types;

use crate::file::blend_loader::load_blend_file;
use crate::r#mod::DemoTool;
use crate::render::vulkan_window::VulkanWindow;
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    let object = load_blend_file("app/naty/File.blend")?;

    let mut app = DemoTool::new(object);
    // let window = VulkanWindow::new();
    // app.load_model(&window.context, &window.app_context, object)?;
    //
    // window.main_loop(app);

    VulkanWindow::new(app).run();

    Ok(())
}
