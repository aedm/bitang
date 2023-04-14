mod binary_file_cache;
mod chart_file;
pub mod file_hash_cache;
mod project_file;
pub mod resource_repository;
mod shader_loader;

use crate::control::controls::ControlRepository;
use anyhow::{Context, Result};
use tracing::{error, info};

pub fn save_controls(repository: &ControlRepository) -> Result<()> {
    let ron = ron::ser::to_string_pretty(repository, ron::ser::PrettyConfig::default())?;
    std::fs::write("app/controls.ron", ron).context("Failed to save controls")?;
    info!("Saved controls to 'app/controls.ron'.");
    Ok(())
}

pub fn load_controls() -> ControlRepository {
    let path = "app/controls.ron";
    let Ok(ron) = std::fs::read_to_string(path) else {
        info!("No controls file found at '{}'.", path);
        return ControlRepository::default();
    };
    let Ok(controls) = ron::de::from_str(&ron) else {
        error!("Failed to parse controls file '{}'.", path);
        return ControlRepository::default();
    };
    info!("Loaded controls from '{}'.", path);
    controls
}
