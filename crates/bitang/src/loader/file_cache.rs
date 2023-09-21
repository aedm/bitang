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
    /// Hash of the file content
    pub hash: ContentHash,

    /// The file content
    pub content: Vec<u8>,
}

pub struct FileCache {
    /// Stores the content of files
    cache: AsyncCache<PathBuf, FileCacheEntry>,

    /// Stores every path during the current document loading cycle
    paths_accessed_in_loading_cycle: Mutex<AHashSet<PathBuf>>,

    /// True if we encountered a missing file during loading
    pub has_missing_files: AtomicBool,

    /// The app's working directory
    current_dir: PathBuf,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            cache: AsyncCache::new(),
            paths_accessed_in_loading_cycle: Mutex::new(AHashSet::new()),
            has_missing_files: AtomicBool::new(false),
            current_dir: env::current_dir().unwrap(),
        }
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
            let mut load_cycle_paths = self.paths_accessed_in_loading_cycle.lock().await;
            load_cycle_paths.insert(absolute_path.clone());
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

/// Takes care of file change events
pub struct FileManager {
    pub file_cache: Arc<FileCache>,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,

    watched_paths: AHashSet<PathBuf>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
}

impl FileManager {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        Self {
            file_cache: Arc::new(FileCache::new()),
            file_change_events: receiver,
            file_watcher: watcher,
            watched_paths: AHashSet::new(),
        }
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

    pub async fn update_watchers(&mut self) {
        let mut paths = self.file_cache.paths_accessed_in_loading_cycle.lock().await;

        // Best effort: if a file is missing, creating a watcher will fail
        for path in self.watched_paths.difference(&paths) {
            if let Some(path) = path.to_str() {
                trace!("Unwatching: {:?}", path.replace('\\', "/"));
            }
            let _ = self.file_watcher.unwatch(path);
        }
        for path in paths.difference(&self.watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Watching: {:?}", path.replace('\\', "/"));
            }
            let _ = self.file_watcher.watch(path, RecursiveMode::NonRecursive);
        }
        self.watched_paths = mem::take(&mut paths);
    }

    pub fn has_missing_files(&self) -> bool {
        self.file_cache.has_missing_files.load(Ordering::Relaxed)
    }
}
