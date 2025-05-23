use std::sync::Arc;

use anyhow::Result;
use tokio::task::spawn_blocking;
use tracing::trace;

use crate::engine::GpuContext;
use crate::loader::async_cache::{AsyncCache, LoadFuture};
use crate::loader::file_cache::{ContentHash, FileCache, FileCacheEntry};
use crate::loader::resource_path::ResourcePath;

type LoaderFunc<T> =
    fn(context: &Arc<GpuContext>, blob: &[u8], resource_name: &str) -> Result<Arc<T>>;

/// Async cache for CPU bound resources.
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

    pub async fn load(&self, context: &Arc<GpuContext>, path: &ResourcePath) -> Result<Arc<T>> {
        let file_hash_cache = self.file_hash_cache.clone();
        let cache_entry = file_hash_cache.get(path).await?;
        let hash = cache_entry.hash;
        let loader_func = self.loader_func;
        let context = context.clone();
        let path_clone = path.clone();
        let async_loader = async move {
            let sync_loader = move || {
                let FileCacheEntry { hash: _, content } = cache_entry.as_ref();
                let now = std::time::Instant::now();
                let resource = loader_func(&context, content, &path_clone.file_name)?;
                trace!("Loading {path_clone:?} took {:?}", now.elapsed());
                Ok(resource)
            };
            // Run the loader function in a blocking thread pool.
            spawn_blocking(sync_loader).await?
        };
        self.resource_cache.get(format!("path:{path:?}"), hash, async_loader).await
    }

    pub fn get_future(
        self: &Arc<Self>,
        context: &Arc<GpuContext>,
        path: &ResourcePath,
    ) -> LoadFuture<T> {
        let self_clone = self.clone();
        let context = context.clone();
        let path = path.clone();
        LoadFuture::new(format!("resource:{path:?}"), async move {
            self_clone.load(&context, &path).await
        })
    }

    pub fn display_load_errors(&self) {
        self.resource_cache.display_load_errors();
    }

    pub fn start_load_cycle(&self) {
        self.resource_cache.start_load_cycle();
    }
}
