use crate::loader::cache::Cache;
use crate::loader::concurrent_cache::ConcurrentCache;
use crate::loader::{compute_hash, ResourcePath};
use ahash::AHashSet;
use anyhow::{bail, Result};
use dashmap::DashSet;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::{env, mem};
use threadpool::ThreadPool;
use tracing::{debug, error, trace};

pub type ContentHash = u64;

#[derive(Clone)]
pub struct FileCacheEntry {
    pub hash: ContentHash,
    pub content: Vec<u8>,
}

pub struct WatchedPaths {
    watched_paths: AHashSet<PathBuf>,

    // Stores every path during the current document loading cycle
    new_watched_paths: AHashSet<PathBuf>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
}

impl WatchedPaths {
    pub fn update_watchers(&mut self) {
        // Best effort: if a file is missing, creating a watcher will fail
        for path in self.watched_paths.difference(&self.new_watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Unwatching: {:?}", path.replace('\\', "/"));
            }
            let _ = self.file_watcher.unwatch(path);
        }
        for path in self.new_watched_paths.difference(&self.watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Watching: {:?}", path.replace('\\', "/"));
            }
            let _ = self.file_watcher.watch(path, RecursiveMode::NonRecursive);
        }
        self.watched_paths = mem::take(&mut self.new_watched_paths);
    }
}

pub struct FileCache {
    current_dir: PathBuf,
    cache: ConcurrentCache<PathBuf, FileCacheEntry>,

    file_change_events: Receiver<Result<notify::Event, notify::Error>>,

    paths: Mutex<WatchedPaths>,

    // Did we encounter a missing file during loading?
    pub has_missing_files: AtomicBool,

    // Loader thread pool
    thread_pool: Arc<ThreadPool>,
}

impl FileCache {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        let thread_pool = Arc::new(threadpool::Builder::new().build());
        Self {
            // cache_map: HashMap::new(),
            cache: ConcurrentCache::new(thread_pool.clone()),
            file_change_events: receiver,
            paths: Mutex::new(WatchedPaths {
                watched_paths: AHashSet::new(),
                new_watched_paths: AHashSet::new(),
                file_watcher: watcher,
            }),
            current_dir: env::current_dir().unwrap(),
            has_missing_files: AtomicBool::new(false),
            thread_pool,
        }
    }

    pub fn handle_file_changes(&self) -> bool {
        let mut has_changes = false;
        for res in self.file_change_events.try_iter() {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        debug!("File change detected: {:?}", path);
                        self.cache.remove(&path);
                    }
                    has_changes = true;
                }
                Err(e) => error!("watch error: {:?}", e),
            }
        }
        has_changes
    }

    pub fn update_watchers(&self) {
        let mut paths = self.paths.lock().unwrap();
        paths.update_watchers();
    }

    pub fn prepare_loading_cycle(&self) {
        self.has_missing_files.store(false, Ordering::Relaxed);
    }

    pub fn get(&self, path: &ResourcePath, _store_content: bool) -> Result<Arc<FileCacheEntry>> {
        let path_string = path.to_string();
        let absolute_path = self.to_absolute_path(&path_string);

        {
            let mut paths = self.paths.lock().unwrap();
            paths.new_watched_paths.insert(absolute_path.clone());
        }

        let loader = self.cache.load(absolute_path.clone(), move || {
            debug!("Reading file: '{path_string}'");
            let Ok(content) = std::fs::read(absolute_path) else {
                bail!("Failed to read file: '{path_string}'");
            };
            Ok(Arc::new(FileCacheEntry {
                hash: compute_hash(&content),
                content,
            }))
        });
        let value = loader.get();
        if value.is_err() {
            self.has_missing_files.store(true, Ordering::Relaxed);
        }
        value
    }

    fn to_absolute_path(&self, path: &str) -> PathBuf {
        let path = std::path::Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current_dir.join(path)
        }
    }
}
