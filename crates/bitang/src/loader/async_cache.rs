use anyhow::{anyhow, Result};
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use dashmap::{DashMap, DashSet};
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tracing::error;

struct LoadFutureInner<T> {
    value: Option<Arc<Result<Arc<T>>>>,
    handle: Option<JoinHandle<Result<Arc<T>>>>,
}

/// A shareable future that loads a resource
pub struct LoadFuture<T> {
    inner: Arc<Mutex<LoadFutureInner<T>>>,
}

impl<T> Clone for LoadFuture<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> PartialEq for LoadFuture<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T> Eq for LoadFuture<T> {}

impl<T> Hash for LoadFuture<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.inner), state);
    }
}

impl<T: Send + Sync + 'static> LoadFuture<T> {
    /// Executes the future and returns a LoadFuture that resolves to the result.
    pub fn new<F: Future<Output = Result<Arc<T>>> + Send + 'static>(func: F) -> Self {
        let join_handle = tokio::spawn(func);
        let inner = Arc::new(Mutex::new(LoadFutureInner {
            value: None,
            handle: Some(join_handle),
        }));
        Self { inner }
    }

    /// Creates a LoadFuture that is already resolved to the given value.
    pub fn new_from_value(value: Arc<T>) -> Self {
        let inner = Arc::new(Mutex::new(LoadFutureInner {
            value: Some(Arc::new(Ok(value))),
            handle: None,
        }));
        Self { inner }
    }

    /// Waits for the future if it is not already resolved.
    async fn resolve(inner: &mut MutexGuard<'_, LoadFutureInner<T>>) {
        if let Some(join_handle) = inner.handle.take() {
            match join_handle.await {
                Ok(result) => inner.value = Some(Arc::new(result)),
                Err(err) => {
                    inner.value = Some(Arc::new(Err(anyhow::anyhow!(
                        "Unhandled load error: {err:?}"
                    ))))
                }
            }
        }
    }

    /// Resolves the future and returns its value.
    pub async fn get(&self) -> Result<Arc<T>> {
        let mut inner = self.inner.lock().await;
        Self::resolve(&mut inner).await;
        match inner.value.as_ref().unwrap().as_ref() {
            Ok(value) => Ok(value.clone()),
            Err(err) => Err(anyhow!("Error loading value: {err:?}")),
        }
    }

    /// Displays the root case of a load error.
    async fn display_load_error(&self) {
        let mut inner = self.inner.lock().await;
        Self::resolve(&mut inner).await;
        match inner.value.as_ref().unwrap().as_ref() {
            Ok(_) => {}
            Err(err) => {
                error!("Error loading value: {err:?}");
            }
        }
    }
}

/// A cache that loads resources asynchronously.
/// For every key, only the first load operation is executed and its result is shared between all requests with the same key.
pub struct AsyncCache<Key: Hash + Eq + Clone, Value: Send + Sync> {
    items: DashMap<Key, LoadFuture<Value>>,
    accessed_in_current_load_cycle: DashSet<LoadFuture<Value>>,
}

impl<Key: Hash + Eq + Clone, Value: Send + Sync + 'static> AsyncCache<Key, Value> {
    pub fn new() -> Self {
        Self {
            items: DashMap::new(),
            accessed_in_current_load_cycle: DashSet::new(),
        }
    }

    /// Returns a shareable future for a particular cache key. Executes the loader if not cached.
    pub fn load<F: Future<Output = Result<Arc<Value>>> + Send + 'static>(
        &self,
        key: Key,
        loader: F,
    ) -> LoadFuture<Value> {
        let future = match self.items.entry(key) {
            Occupied(entry) => entry.get().clone(),
            Vacant(entry) => {
                let loading = LoadFuture::new(loader);
                entry.insert(loading.clone());
                loading
            }
        };
        self.accessed_in_current_load_cycle.insert(future.clone());
        future
    }

    /// Returns the value for a particular cache key. Loads it if not cached.
    pub async fn get<F: Future<Output = Result<Arc<Value>>> + Send + 'static>(
        &self,
        key: Key,
        loader: F,
    ) -> Result<Arc<Value>> {
        let future = self.load(key, loader);
        future.get().await
    }

    /// Call this before starting a new loading cycle.
    pub fn reset_load_cycle(&mut self) {
        self.accessed_in_current_load_cycle.clear();
    }

    /// Removes a key from the cache.
    pub fn remove(&self, key: &Key) {
        self.items.remove(key);
    }

    /// Displays the root cause of all load errors that occurred during the current loading cycle.
    pub async fn display_load_errors(&self) {
        for future in self.accessed_in_current_load_cycle.iter() {
            future.display_load_error().await;
        }
    }
}
