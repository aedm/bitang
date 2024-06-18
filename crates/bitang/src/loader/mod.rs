use ahash::AHasher;
use std::hash::Hasher;
pub mod async_cache;
pub mod file_cache;
mod gltf_loader;
pub mod project_loader;
pub mod resource_cache;
pub mod resource_path;
pub mod resource_repository;
pub mod shader_cache;
pub mod shader_compiler;

/// Project file name
const PROJECT_FILE_NAME: &str = "project.ron";

/// Folder for charts
pub const CHARTS_FOLDER: &str = "charts";

/// Chart file name
const CHART_FILE_NAME: &str = "chart.ron";

/// Computes the hash of a binary blob
pub fn compute_hash(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
