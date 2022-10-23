mod file;
mod types;
mod render;
mod draw;

use anyhow::Result;
use crate::file::blend_loader::load_blend_file;
use crate::types::{Mesh, Object, Vertex};

fn main() -> Result<()> {
    let object = load_blend_file("app/file.blend")?;
    // println!("{:#?}", object);

    draw::main();

    Ok(())
}

