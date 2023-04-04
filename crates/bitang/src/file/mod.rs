mod binary_file_cache;
mod chart_file;
pub mod file_hash_cache;
pub mod resource_repository;
mod shader_loader;

use crate::control::controls::Controls;
use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{error, info};

pub fn save_controls(controls: &Controls) -> Result<()> {
    let ron = ron::ser::to_string_pretty(controls, ron::ser::PrettyConfig::default())?;
    std::fs::write("app/controls.ron", ron).context("Failed to save controls")?;
    info!("Saved controls to 'app/controls.ron'.");
    Ok(())
}

pub fn load_controls() -> Controls {
    let path = "app/controls.ron";
    let Ok(ron) = std::fs::read_to_string(path) else {
        info!("No controls file found at '{}'.", path);
        return Controls::default();
    };
    let Ok(controls) = ron::de::from_str(&ron) else {
        error!("Failed to parse controls file '{}'.", path);
        return Controls::default();
    };
    info!("Loaded controls from '{}'.", path);
    controls
}
