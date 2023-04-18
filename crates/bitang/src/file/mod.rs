mod binary_file_cache;
mod chart_file;
pub mod file_hash_cache;
mod project_file;
pub mod resource_repository;
mod shader_loader;

use crate::control::controls::ControlRepository;
use anyhow::{Context, Result};
use tracing::{error, info, trace};

// The root folder for all content.
pub const ROOT_FOLDER: &str = "content";

// pub fn save_controls(repository: &ControlRepository) -> Result<()> {
//     let ron = ron::ser::to_string_pretty(repository, ron::ser::PrettyConfig::default())?;
//     std::fs::write("app/controls.ron", ron).context("Failed to save controls")?;
//     info!("Saved controls to 'app/controls.ron'.");
//     Ok(())
// }

// pub fn load_controls() -> ControlRepository {
//     unimplemented!("load_controls")
//
//     // let path = "app/controls.ron";
//     // let Ok(ron) = std::fs::read_to_string(path) else {
//     //     info!("No controls file found at '{}'.", path);
//     //     return ControlRepository::default();
//     // };
//     // let Ok(controls) = ron::de::from_str(&ron) else {
//     //     error!("Failed to parse controls file '{}'.", path);
//     //     return ControlRepository::default();
//     // };
//     // info!("Loaded controls from '{}'.", path);
//     // controls
// }

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ResourcePath {
    pub directory: String,
    pub file_name: String,
}

impl ResourcePath {
    fn new(directory: &str, file_name: &str) -> Self {
        Self {
            directory: directory.to_owned(),
            file_name: file_name.to_owned(),
        }
    }

    fn to_string(&self) -> String {
        format!("{ROOT_FOLDER}/{}/{}", self.directory, self.file_name)
    }

    fn relative_path(&self, file_name: &str) -> Self {
        let parts = file_name.split('/').collect::<Vec<_>>();
        let directory = if file_name.starts_with('/') {
            parts[1..parts.len() - 1].join("/")
        } else if parts.len() > 1 {
            format!("{}/{}", self.directory, parts[..parts.len() - 1].join("/"))
        } else {
            self.directory.clone()
        };
        Self {
            directory,
            file_name: parts.last().unwrap().to_string(),
        }
    }
}
