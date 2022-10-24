mod draw;
mod file;
mod render;
mod types;

use crate::draw::VulkanRenderer;
use crate::file::blend_loader::load_blend_file;
use crate::types::{Mesh, Object, Vertex};
use anyhow::Result;

fn main() -> Result<()> {
    let _object = load_blend_file("app/file.blend")?;
    // println!("{:#?}", object);

    VulkanRenderer::new().main_loop();

    Ok(())
}
