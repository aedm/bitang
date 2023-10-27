use ahash::AHasher;
use anyhow::{ensure, Result};
use std::fmt;
use std::hash::Hasher;

pub mod async_cache;
pub mod file_cache;
pub mod project_loader;
pub mod resource_cache;
pub mod resource_repository;
pub mod shader_compiler;
pub mod shader_loader;

/// The root folder for all content.
pub const ROOT_FOLDER: &str = "app";

/// Project file name
const PROJECT_FILE_NAME: &str = "project.ron";

/// Folder for charts
pub const CHARTS_FOLDER: &str = "charts";

/// Chart file name
const CHART_FILE_NAME: &str = "chart.ron";

/// The path of a resource file
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct ResourcePath {
    pub directory: String,
    pub file_name: String,
}

impl ResourcePath {
    pub fn new(directory: &str, file_name: &str) -> Self {
        Self {
            directory: directory.to_owned(),
            file_name: file_name.to_owned(),
        }
    }

    /// Returns a new ResourcePath for a file relative to the current one.
    /// Path starting with '/' are relative to the root folder.
    pub fn relative_path(&self, file_name: &str) -> Self {
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

    /// From a string like "app/folder/file.txt". Must start with the root folder,
    /// it's the reverse of `to_string`.
    pub fn from_str(path: &str) -> Result<Self> {
        let parts = path.split('/').collect::<Vec<_>>();
        ensure!(
            parts[0] == ROOT_FOLDER,
            "Path must start with '{}', got '{}'",
            ROOT_FOLDER,
            parts[0]
        );
        ensure!(
            parts.len() > 1,
            "Path must point to a file inside the app folder."
        );
        let directory = parts[1..parts.len() - 1].join("/");
        Ok(Self {
            directory,
            file_name: parts.last().unwrap().to_string(),
        })
    }
}

impl fmt::Display for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{ROOT_FOLDER}/{}/{}", self.directory, self.file_name)
    }
}

impl fmt::Debug for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}/{}'", self.directory, self.file_name)
    }
}

/// Computes the hash of a binary blob
pub fn compute_hash(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
