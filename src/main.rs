// mod draw;
mod file;
// mod gui;
mod render;
mod types;

// use crate::draw::VulkanApp;
use crate::file::blend_loader::load_blend_file;
// use crate::gui::Gui;
use crate::render::vulkan_window::VulkanWindow;
// use crate::render::{DemoApp, VulkanRenderer};
use crate::render::DemoApp;
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    let object = load_blend_file("app/naty/File.blend")?;

    let window = VulkanWindow::new();
    let mut app = DemoApp::new();
    app.load_model(&window.context, &window.app_context, object)?;

    window.main_loop(app);

    Ok(())
}
