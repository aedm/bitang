// use ahash::{AHashMap, AHasher};
// use anyhow::{anyhow, Result};
// use dashmap::mapref::entry::Entry::{Occupied, Vacant};
// use dashmap::{DashMap, DashSet};
// use std::error::Error;
// use std::hash::Hash;
// use std::sync::{mpsc, Arc, Mutex, MutexGuard};
// use threadpool::ThreadPool;
// use tracing::error;
//
// struct LoadingInner<T> {
//     value: Option<Arc<Result<Arc<T>>>>,
//     receiver: Option<mpsc::Receiver<Arc<Result<Arc<T>>>>>,
// }
//
// /// A future that loads a value in a background thread.
// pub struct Loading<T> {
//     inner: Arc<Mutex<LoadingInner<T>>>,
// }
//
// impl<T> Clone for Loading<T> {
//     fn clone(&self) -> Self {
//         Self {
//             inner: self.inner.clone(),
//         }
//     }
// }
//
// impl<T> PartialEq for Loading<T> {
//     fn eq(&self, other: &Self) -> bool {
//         Arc::ptr_eq(&self.inner, &other.inner)
//     }
// }
//
// impl<T> Eq for Loading<T> {}
//
// impl<T> Hash for Loading<T> {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         std::ptr::hash(Arc::as_ptr(&self.inner), state);
//     }
// }
//
// impl<T: Send + Sync + 'static> Loading<T> {
//     fn new<F: FnOnce(mpsc::Sender<Arc<Result<Arc<T>>>>) + Send + 'static>(
//         thread_pool: &ThreadPool,
//         func: F,
//     ) -> Self {
//         let (tx, rx) = mpsc::channel();
//         thread_pool.execute(move || func(tx));
//         let inner = Arc::new(Mutex::new(LoadingInner {
//             value: None,
//             receiver: Some(rx),
//         }));
//         Self { inner }
//     }
//
//     fn resolve(inner: &mut MutexGuard<LoadingInner<T>>) {
//         if let Some(receiver) = inner.receiver.take() {
//             match receiver.recv() {
//                 Ok(value) => inner.value = Some(value),
//                 Err(err) => {
//                     inner.value = Some(Arc::new(Err(anyhow::anyhow!(
//                         "Unhandled load error: {err:?}"
//                     ))))
//                 }
//             }
//         }
//     }
//
//     // Returns the value if it is already loaded, otherwise blocks until it is loaded.
//     pub fn get(&self) -> Result<Arc<T>> {
//         let mut inner = self.inner.lock().unwrap();
//         Self::resolve(&mut inner);
//         match inner.value.as_ref().unwrap().as_ref() {
//             Ok(value) => Ok(value.clone()),
//             Err(err) => Err(anyhow!("Error loading value: {err:?}")),
//         }
//     }
//
//     // Displays the root case of a load error.
//     fn display_load_error(&self) {
//         let mut inner = self.inner.lock().unwrap();
//         Self::resolve(&mut inner);
//         match inner.value.as_ref().unwrap().as_ref() {
//             Ok(_) => {}
//             Err(err) => {
//                 error!("Error loading value: {err:?}");
//             }
//         }
//     }
// }
//
// pub struct ConcurrentCache<Key: Hash + Eq + Clone, Value: Send + Sync> {
//     items: DashMap<Key, Loading<Value>>,
//     thread_pool: Arc<ThreadPool>,
//     accessed_in_current_load_cycle: DashSet<Loading<Value>>,
// }
//
// impl<Key: Hash + Eq + Clone, Value: Send + Sync + 'static> ConcurrentCache<Key, Value> {
//     pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
//         Self {
//             items: DashMap::new(),
//             thread_pool,
//             accessed_in_current_load_cycle: DashSet::new(),
//         }
//     }
//
//     pub fn load<F: FnOnce() -> Result<Arc<Value>> + Send + 'static>(
//         &self,
//         key: Key,
//         loader: F,
//     ) -> Loading<Value> {
//         let future = match self.items.entry(key) {
//             Occupied(entry) => entry.get().clone(),
//             Vacant(entry) => {
//                 let loading = Loading::new(&self.thread_pool, |tx| {
//                     let value = loader();
//                     tx.send(Arc::new(value)).unwrap();
//                 });
//                 entry.insert(loading.clone());
//                 loading
//             }
//         };
//         self.accessed_in_current_load_cycle.insert(future.clone());
//         future
//     }
//
//     pub fn reset_load_cycle(&mut self) {
//         self.accessed_in_current_load_cycle.clear();
//     }
//
//     pub fn remove(&self, key: &Key) {
//         self.items.remove(key);
//     }
//
//     pub fn display_load_errors(&self) {
//         for future in self.accessed_in_current_load_cycle.iter() {
//             future.display_load_error();
//         }
//     }
// }
