use anyhow::{anyhow, Result};
use dashmap::mapref::entry::Entry::{Occupied, Vacant};
use dashmap::{DashMap, DashSet};
use futures::future::join_all;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tracing::error;

pub trait ResourceFuture<T>: Future<Output = Result<Arc<T>>> + Sized + Send + 'static {}

struct LoadFutureInner<T> {
    value: Option<Arc<Result<Arc<T>>>>,
    handle: Option<JoinHandle<Result<Arc<T>>>>,
}

/// A future that loads a value in a background thread.
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
    // fn new<F: FnOnce() -> Result<Arc<T>> + Send + 'static>(func: F) -> Self {
    pub fn new<F: Future<Output = Result<Arc<T>>> + Send + 'static>(func: F) -> Self {
        let join_handle = tokio::spawn(func);
        let inner = Arc::new(Mutex::new(LoadFutureInner {
            value: None,
            handle: Some(join_handle),
        }));
        Self { inner }
    }

    pub fn new_from_value(value: Arc<T>) -> Self {
        let inner = Arc::new(Mutex::new(LoadFutureInner {
            value: Some(Arc::new(Ok(value))),
            handle: None,
        }));
        Self { inner }
    }

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

    // Returns the value if it is already loaded, otherwise blocks until it is loaded.
    pub async fn get(&self) -> Result<Arc<T>> {
        let mut inner = self.inner.lock().await;
        Self::resolve(&mut inner).await;
        match inner.value.as_ref().unwrap().as_ref() {
            Ok(value) => Ok(value.clone()),
            Err(err) => Err(anyhow!("Error loading value: {err:?}")),
        }
    }

    // Displays the root case of a load error.
    async fn display_load_error(&self) {
        let mut inner = self.inner.lock().await;
        Self::resolve(&mut inner);
        match inner.value.as_ref().unwrap().as_ref() {
            Ok(_) => {}
            Err(err) => {
                error!("Error loading value: {err:?}");
            }
        }
    }
}

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

    pub async fn get<F: Future<Output = Result<Arc<Value>>> + Send + 'static>(
        &self,
        key: Key,
        loader: F,
    ) -> Result<Arc<Value>> {
        let future = self.load(key, loader);
        future.get().await
    }

    pub fn reset_load_cycle(&mut self) {
        self.accessed_in_current_load_cycle.clear();
    }

    pub fn remove(&self, key: &Key) {
        self.items.remove(key);
    }

    pub fn display_load_errors(&self) {
        for future in self.accessed_in_current_load_cycle.iter() {
            future.display_load_error();
        }
    }
}

trait Flomp<T>: Future<Output = Result<Arc<T>>> + Sized {}

// pub async fn load_all<T, I: IntoIterator<Item = dyn Future<Output = Result<Arc<T>>>> + 'static>(
//     iter: I,
// ) -> Result<Vec<Arc<T>>> {
//     join_all(iter)
//         .await
//         .into_iter()
//         .collect::<Result<Vec<Arc<T>>>>()
// }
