use crate::file::file_hash_cache::{FileContentHash, FileHashCache};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{Context, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

type LoaderFunc<T> = fn(&VulkanContext, &[u8]) -> Result<T>;

pub struct BinaryFileCache<T> {
    file_hash_cache: Rc<RefCell<FileHashCache>>,
    resource_cache: HashMap<FileContentHash, T>,
    loader_func: LoaderFunc<T>,
}

impl<T> BinaryFileCache<T> {
    pub fn new(file_hash_cache: &Rc<RefCell<FileHashCache>>, loader_func: LoaderFunc<T>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            resource_cache: HashMap::new(),
            loader_func,
        }
    }

    pub fn get_or_load(&mut self, context: &VulkanContext, path: &PathBuf) -> Result<&T> {
        let (hash, source) = self.file_hash_cache.borrow_mut().get(path)?;

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
