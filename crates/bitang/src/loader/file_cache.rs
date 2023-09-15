use crate::loader::cache::Cache;
use crate::loader::{compute_hash, ResourcePath};
use ahash::AHashSet;
use anyhow::{bail, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::{env, mem};
use tracing::{debug, error, trace};

pub type ContentHash = u64;

#[derive(Clone)]
pub struct FileCacheEntry {
    pub hash: ContentHash,
    pub content: Vec<u8>,
}

pub struct FileCache {
    current_dir: PathBuf,
    cache: Cache<PathBuf, FileCacheEntry>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,
    watched_paths: AHashSet<PathBuf>,

    // Stores every path during the current document loading cycle
    new_watched_paths: AHashSet<PathBuf>,

    // Did we encounter a missing file during loading?
    pub has_missing_files: bool,
}

impl FileCache {
    pub fn new() -> Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender)?;
        Ok(Self {
            // cache_map: HashMap::new(),
            cache: Cache::new(),
            file_watcher: watcher,
            file_change_events: receiver,
            watched_paths: AHashSet::new(),
            new_watched_paths: AHashSet::new(),
            current_dir: env::current_dir()?,
            has_missing_files: false,
        })
    }

    pub fn handle_file_changes(&mut self) -> bool {
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

    pub fn prepare_loading_cycle(&mut self) {
        self.has_missing_files = false;
    }

    pub fn get(&mut self, path: &ResourcePath, _store_content: bool) -> Result<&FileCacheEntry> {
        let path_string = path.to_string();
        let absolute_path = self.to_absolute_path(&path_string);
        self.new_watched_paths.insert(absolute_path.clone());

        self.cache.get_or_try_insert_with_key(absolute_path, |key| {
            debug!("Reading file: '{path_string}'");
            let Ok(content) = std::fs::read(key) else {
                self.has_missing_files = true;
                bail!("Failed to read file: '{path_string}'");
            };
            Ok(FileCacheEntry {
                hash: compute_hash(&content),
                content,
            })
        })
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
