mod binary_file_cache;
pub mod file_hash_cache;
pub mod resource_repository;
mod shader_loader;

use crate::control::controls::Controls;
use anyhow::{Context, Result};

pub fn save_controls(controls: &Controls) -> Result<()> {
    let ron = ron::ser::to_string_pretty(controls, ron::ser::PrettyConfig::default())?;
    std::fs::write("app/controls.ron", ron).context("Failed to save controls")?;
    Ok(())
}

pub fn load_controls() -> Result<Controls> {
    let ron = std::fs::read_to_string("app/controls.ron").context("Failed to load controls")?;
    let controls = ron::de::from_str(&ron).context("Failed to parse controls")?;
    Ok(controls)
}
