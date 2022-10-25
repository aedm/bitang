mod draw;
mod file;
mod render;
mod types;

use crate::draw::VulkanApp;
use crate::file::blend_loader::load_blend_file;
use crate::render::DemoApp;
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    let _object = load_blend_file("app/file.blend")?;
    // println!("{:#?}", object);

    let mut va = VulkanApp::new();
    let mut app = DemoApp::new(&va.renderer);
    va.main_loop(app);

    Ok(())
}
