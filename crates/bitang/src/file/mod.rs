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
pub const ROOT_FOLDER: &str = "app";

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
