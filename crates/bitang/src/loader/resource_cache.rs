use crate::loader::async_cache::{AsyncCache, LoadFuture};
use crate::loader::file_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::loader::ResourcePath;
use crate::render::vulkan_window::VulkanContext;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

type LoaderFunc<T> =
    fn(context: &Arc<VulkanContext>, blob: &[u8], resource_name: &str) -> Result<Arc<T>>;

/// Cache mechanism for file-based resources like images and meshes.
pub struct ResourceCache<T: Send + Sync + 'static> {
    file_hash_cache: Arc<FileCache>,
    resource_cache: AsyncCache<ContentHash, T>,
    loader_func: LoaderFunc<T>,
}

impl<T: Send + Sync> ResourceCache<T> {
    pub fn new(file_hash_cache: &Arc<FileCache>, loader_func: LoaderFunc<T>) -> Self {
        Self {
            file_hash_cache: file_hash_cache.clone(),
            resource_cache: AsyncCache::new(),
            loader_func,
        }
    }

    pub async fn load(&self, context: &Arc<VulkanContext>, path: &ResourcePath) -> Result<Arc<T>> {
        let file_hash_cache = self.file_hash_cache.clone();
        let cache_entry = file_hash_cache.get(path, true).await?;
        let hash = cache_entry.hash;
        let loader_func = self.loader_func.clone();
        let context = context.clone();
        let path = path.clone();
        let async_loader = async move {
            let FileCacheEntry { hash, content } = cache_entry.as_ref();
            let now = std::time::Instant::now();
            let resource = loader_func(&context, content, &path.file_name)?;
            info!("Loading {} took {:?}", &path.to_string(), now.elapsed());
            Ok(resource)
        };
        self.resource_cache.get(hash, async_loader).await
    }

    pub fn get_future(
        self: &Arc<Self>,
        context: &Arc<VulkanContext>,
        path: &ResourcePath,
    ) -> LoadFuture<T> {
        let self_clone = self.clone();
        let context = context.clone();
        let path = path.clone();
        LoadFuture::new(async move { self_clone.load(&context, &path).await })
    }
}
