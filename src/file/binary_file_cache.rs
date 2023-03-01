use crate::file::file_hash_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::render::vulkan_window::VulkanContext;
use anyhow::{Context, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type LoaderFunc<T> = fn(&VulkanContext, &[u8]) -> Result<T>;

pub struct BinaryFileCache<T> {
    file_hash_cache: Rc<RefCell<FileCache>>,
    resource_cache: HashMap<ContentHash, T>,
    loader_func: LoaderFunc<T>,
}

impl<T> BinaryFileCache<T> {
    pub fn new(file_hash_cache: &Rc<RefCell<FileCache>>, loader_func: LoaderFunc<T>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            resource_cache: HashMap::new(),
            loader_func,
        }
    }

    pub fn get_or_load(&mut self, context: &VulkanContext, path: &str) -> Result<&T> {
        let FileCacheEntry { hash, content } = self.file_hash_cache.borrow_mut().get(path, true)?;

        // TODO: simplify when they fix if-let borrow leaks. https://github.com/rust-lang/rust/issues/21906
        if self.resource_cache.contains_key(&hash) {
            Ok(self.resource_cache.get(&hash).unwrap())
        } else {
            let source = match content {
                Some(source) => source,
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
