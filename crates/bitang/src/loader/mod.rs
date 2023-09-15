use ahash::AHasher;
use std::hash::Hasher;

mod cache;
pub mod file_cache;
pub mod resource_cache;
pub mod resource_repository;
pub mod shader_loader;

pub fn compute_hash(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
