use crate::loader::cache::Cache;
use crate::loader::file_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::loader::ResourcePath;
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::info;

type LoaderFunc<T> =
    fn(context: &Arc<VulkanContext>, blob: &[u8], resource_name: &str) -> Result<T>;

/// Cache mechanism for file-based resources like images and meshes.
pub struct ResourceCache<T> {
    file_hash_cache: Arc<FileCache>,
    resource_cache: Cache<ContentHash, T>,
    loader_func: LoaderFunc<T>,
}

impl<T> ResourceCache<T> {
    pub fn new(file_hash_cache: &Arc<FileCache>, loader_func: LoaderFunc<T>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            resource_cache: Cache::new(),
            loader_func,
        }
    }

    pub fn get_or_load(&mut self, context: &Arc<VulkanContext>, path: &ResourcePath) -> Result<&T> {
        let cache_entry = self.file_hash_cache.get(path, true)?;
        let FileCacheEntry { hash, content } = cache_entry.as_ref();

        self.resource_cache
            .get_or_try_insert_with_key(*hash, |_key| {
                let now = std::time::Instant::now();
                let resource = (self.loader_func)(context, content, &path.file_name)?;
                info!("Loading {} took {:?}", &path.to_string(), now.elapsed());
                Ok(resource)
            })
    }
}
