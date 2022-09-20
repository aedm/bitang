mod file;
mod types;
mod render;

use anyhow::Result;
use crate::file::blend_loader::load_blend_file;
use crate::types::{Mesh, Object, Vertex};

fn main() -> Result<()> {
    let object = load_blend_file("app/file.blend")?;
    println!("{:#?}", object);
    Ok(())
}

