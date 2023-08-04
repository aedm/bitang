use crate::file::file_hash_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::file::ResourcePath;
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::info;

type LoaderFunc<T> = fn(&VulkanContext, &[u8], &str) -> Result<T>;

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

    pub fn get_or_load(&mut self, context: &VulkanContext, path: &ResourcePath) -> Result<&T> {
        let FileCacheEntry { hash, content } = self.file_hash_cache.borrow_mut().get(path, true)?;

        // TODO: simplify when they fix if-let borrow leaks. https://github.com/rust-lang/rust/issues/21906
        if let Entry::Vacant(entry) = self.resource_cache.entry(hash) {
            let now = std::time::Instant::now();
            let source = match content {
                Some(source) => source,
                None => std::fs::read(path.to_string())?,
            };
            let resource = (self.loader_func)(context, &source)?;
            info!("Loading {} took {:?}", &path.to_string(), now.elapsed());
            entry.insert(resource);
        }

        // Unwrap is safe: we just checked that the key exists
        Ok(self.resource_cache.get(&hash).unwrap())
    }
}
