use crate::file::file_hash_cache::{FileContentHash, FileHashCache};
use crate::render::vulkan_window::VulkanContext;
use crate::render::Texture;
use ahash::{AHasher, RandomState};
use anyhow::{Context, Result};
use notify::RecommendedWatcher;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

type LoaderFunc<T> = fn(&VulkanContext, &[u8]) -> Result<T>;

pub struct BinaryFileCache<T> {
    file_hash_cache: FileHashCache,
    resource_cache: HashMap<FileContentHash, T>,
    loader_func: LoaderFunc<T>,
}

impl<T> BinaryFileCache<T> {
    pub fn new(loader_func: LoaderFunc<T>) -> Self {
        Self {
            file_hash_cache: FileHashCache::new(),
            resource_cache: HashMap::new(),
            loader_func,
        }
    }

    pub fn start_load_cycle(&mut self) -> bool {
        self.file_hash_cache.start_load_cycle()
    }

    pub fn end_load_cycle(&mut self) -> Result<()> {
        self.file_hash_cache.end_load_cycle()
    }

    pub fn get_or_load(&mut self, context: &VulkanContext, path: &PathBuf) -> Result<&T> {
        let (hash, source) = self.file_hash_cache.get(path)?;

        // TODO: simplify when they fix if-let borrow leaks. https://github.com/rust-lang/rust/issues/21906
        if self.resource_cache.contains_key(&hash) {
            Ok(self.resource_cache.get(&hash).unwrap())
        } else {
            let source = match source {
                Some(x) => x,
                None => std::fs::read(path)?,
            };
            let resource = (self.loader_func)(context, &source)?;
            self.resource_cache.insert(hash, resource);
            self.resource_cache
                .get(&hash)
                .context("Failed to get resource")
        }
    }
}
