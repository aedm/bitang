use ahash::AHasher;
use std::fmt;
use std::hash::Hasher;

mod cache;
pub mod concurrent_cache;
pub mod file_cache;
pub mod resource_cache;
pub mod resource_repository;
pub mod shader_loader;

/// The root folder for all content.
pub const ROOT_FOLDER: &str = "app";

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
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
}

impl fmt::Display for ResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{ROOT_FOLDER}/{}/{}", self.directory, self.file_name)
    }
}

/// Computes the hash of a binary blob
pub fn compute_hash(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
