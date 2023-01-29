// mod draw;
mod file;
// mod gui;
mod render;
mod types;

// use crate::draw::VulkanApp;
use crate::file::blend_loader::load_blend_file;
// use crate::gui::Gui;
use crate::render::vulkan_renderer::VulkanPainter;
// use crate::render::{DemoApp, VulkanRenderer};
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    // let object = load_blend_file("app/naty/File.blend")?;
    // // println!("{:#?}", object);
    //
    // let mut va = VulkanApp::new();
    // let mut app = DemoApp::new(&va.renderer);
    // app.load_model(&va.renderer, object)?;
    // let mut gui = Gui::new(&va.renderer);
    // va.main_loop(app, gui);

    let renderer = VulkanPainter::new();
    renderer.main_loop();

    Ok(())
}
