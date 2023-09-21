use crate::loader::async_cache::AsyncCache;
use crate::loader::{compute_hash, ResourcePath};
use ahash::AHashSet;
use anyhow::{bail, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::{env, mem};
use tokio::sync::Mutex;
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
}

impl WatchedPaths {
    pub fn update_watchers(&mut self, file_watcher: &mut RecommendedWatcher) {
        // Best effort: if a file is missing, creating a watcher will fail
        for path in self.watched_paths.difference(&self.new_watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Unwatching: {:?}", path.replace('\\', "/"));
            }
            let _ = file_watcher.unwatch(path);
        }
        for path in self.new_watched_paths.difference(&self.watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Watching: {:?}", path.replace('\\', "/"));
            }
            let _ = file_watcher.watch(path, RecursiveMode::NonRecursive);
        }
        self.watched_paths = mem::take(&mut self.new_watched_paths);
    }
}

pub struct FileCache {
    cache: AsyncCache<PathBuf, FileCacheEntry>,

    paths: Mutex<WatchedPaths>,

    // Did we encounter a missing file during loading?
    pub has_missing_files: AtomicBool,

    current_dir: PathBuf,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            // cache_map: HashMap::new(),
            cache: AsyncCache::new(),
            paths: Mutex::new(WatchedPaths {
                watched_paths: AHashSet::new(),
                new_watched_paths: AHashSet::new(),
            }),
            has_missing_files: AtomicBool::new(false),
            current_dir: env::current_dir().unwrap(),
        }
    }

    async fn update_watchers(&self, file_watcher: &mut RecommendedWatcher) {
        let mut paths = self.paths.lock().await;
        paths.update_watchers(file_watcher);
    }

    pub fn prepare_loading_cycle(&self) {
        self.has_missing_files.store(false, Ordering::Relaxed);
    }

    pub async fn get(
        &self,
        path: &ResourcePath,
        _store_content: bool,
    ) -> Result<Arc<FileCacheEntry>> {
        let path_string = path.to_string();
        let absolute_path = self.to_absolute_path(&path_string);

        {
            let mut paths = self.paths.lock().await;
            paths.new_watched_paths.insert(absolute_path.clone());
        }

        let value = self
            .cache
            .get(absolute_path.clone(), async move {
                debug!("Reading file: '{path_string}'");
                let Ok(content) = tokio::fs::read(absolute_path).await else {
                bail!("Failed to read file: '{path_string}'");
            };
                Ok(Arc::new(FileCacheEntry {
                    hash: compute_hash(&content),
                    content,
                }))
            })
            .await;
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

pub struct FileLoader {
    pub file_cache: Arc<FileCache>,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
}

impl FileLoader {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        Self {
            file_cache: Arc::new(FileCache::new()),
            file_change_events: receiver,
            file_watcher: watcher,
        }
    }

    pub async fn update_watchers(&mut self) {
        self.file_cache
            .update_watchers(&mut self.file_watcher)
            .await;
    }

    /// Returns true if there were any file changes
    pub fn handle_file_changes(&self) -> bool {
        let mut has_changes = false;
        for res in self.file_change_events.try_iter() {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        debug!("File change detected: {:?}", path);
                        self.file_cache.cache.remove(&path);
                    }
                    has_changes = true;
                }
                Err(e) => error!("watch error: {:?}", e),
            }
        }
        has_changes
    }

    pub fn has_missing_files(&self) -> bool {
        self.file_cache.has_missing_files.load(Ordering::Relaxed)
    }
}
