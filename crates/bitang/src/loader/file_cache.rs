use std::mem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use ahash::AHashSet;
use anyhow::{bail, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;
use tracing::{debug, error, trace};

use crate::loader::async_cache::AsyncCache;
use crate::loader::compute_hash;
use crate::loader::resource_path::ResourcePath;

pub type ContentHash = u64;

#[derive(Clone)]
pub struct FileCacheEntry {
    /// Hash of the file content
    pub hash: ContentHash,

    /// The file content
    pub content: Vec<u8>,
}

pub struct FileCache {
    /// Every cached file is somewhere under the root path
    pub root_path: Arc<PathBuf>,

    /// Stores the content of files
    cache: AsyncCache<PathBuf, FileCacheEntry>,

    /// Stores every path during the current document loading cycle
    paths_accessed_in_loading_cycle: Mutex<AHashSet<PathBuf>>,

    /// True if we encountered a missing file during loading
    pub has_missing_files: AtomicBool,
}

impl FileCache {
    pub fn new(root_path: &Arc<PathBuf>) -> Self {
        Self {
            root_path: Arc::clone(root_path),
            cache: AsyncCache::new(),
            paths_accessed_in_loading_cycle: Mutex::new(AHashSet::new()),
            has_missing_files: AtomicBool::new(false),
        }
    }

    pub fn start_load_cycle(&self) {
        self.has_missing_files.store(false, Ordering::Relaxed);
    }

    pub async fn get(&self, path: &ResourcePath) -> Result<Arc<FileCacheEntry>> {
        let absolute_path = path.absolute_path()?;
        {
            let mut load_cycle_paths = self.paths_accessed_in_loading_cycle.lock().await;
            load_cycle_paths.insert(absolute_path.clone());
        }

        let path = path.clone();
        let value = self
            .cache
            .get(
                format!("file:{:?}", absolute_path),
                absolute_path.clone(),
                async move {
                    debug!("Reading file: {path:?}");
                    let Ok(content) = tokio::fs::read(&absolute_path).await else {
                        bail!("Failed to read file: {path:?}");
                    };
                    let file_cache_entry = spawn_blocking(move || FileCacheEntry {
                        hash: compute_hash(&content),
                        content,
                    })
                    .await?;
                    Ok(Arc::new(file_cache_entry))
                },
            )
            .await;
        if value.is_err() {
            self.has_missing_files.store(true, Ordering::Relaxed);
        }
        value
    }

    /// Adds a path to the list of paths accessed during the current loading cycle
    /// without reading the file. The shader compiler doesn't use the file cache,
    /// but we need to keep track of the paths it accessed so we can watch them.
    pub async fn add_accessed_path(&self, path_buf: PathBuf) {
        let mut load_cycle_paths = self.paths_accessed_in_loading_cycle.lock().await;
        load_cycle_paths.insert(path_buf);
    }
}

/// Takes care of file change events
pub struct FileChangeHandler {
    pub file_cache: Arc<FileCache>,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,
    watched_paths: AHashSet<PathBuf>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
}

impl FileChangeHandler {
    pub fn new(file_cache: &Arc<FileCache>) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        Self {
            file_cache: Arc::clone(file_cache),
            file_change_events: receiver,
            file_watcher: watcher,
            watched_paths: AHashSet::new(),
        }
    }

    /// Returns true if there were any file changes
    pub fn handle_file_changes(&self) -> Option<Vec<ResourcePath>> {
        let mut changed_paths = if self.watched_paths.is_empty() {
            // If no file was loaded yet, we can assume there are file changes
            Some(Vec::new())
        } else {
            None
        };
        for res in self.file_change_events.try_iter() {
            match res {
                Ok(event) => {
                    if changed_paths.is_none() {
                        changed_paths = Some(Vec::new());
                    }
                    let changed_paths = changed_paths.as_mut().unwrap();
                    for path in event.paths {
                        trace!("File change detected: {:?}", path);
                        self.file_cache.cache.remove(&path);
                        let resource_path =
                            ResourcePath::from_pathbuf(&self.file_cache.root_path, &path).unwrap();
                        changed_paths.push(resource_path);
                    }
                }
                Err(e) => error!("watch error: {:?}", e),
            }
        }
        changed_paths
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
