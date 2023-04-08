use ahash::AHasher;
use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::{env, mem};
use tracing::{debug, error, trace};

pub type ContentHash = u64;

#[derive(Clone)]
pub struct FileCacheEntry {
    pub hash: ContentHash,
    pub content: Option<Vec<u8>>,
}

pub struct FileCache {
    current_dir: PathBuf,
    cache_map: HashMap<PathBuf, FileCacheEntry>,

    // Listening for file changes
    file_watcher: RecommendedWatcher,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,
    watched_paths: HashSet<PathBuf>,

    // Stores every path during the current document loading cycle
    new_watched_paths: HashSet<PathBuf>,
}

impl FileCache {
    pub fn new() -> Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender)?;
        Ok(Self {
            cache_map: HashMap::new(),
            file_watcher: watcher,
            file_change_events: receiver,
            watched_paths: HashSet::new(),
            new_watched_paths: HashSet::new(),
            current_dir: env::current_dir()?,
        })
    }

    pub fn handle_file_changes(&mut self) -> bool {
        let mut has_changes = false;
        for res in self.file_change_events.try_iter() {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        debug!("File change detected: {:?}", path);
                        self.cache_map.remove(&path);
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
                trace!("Unwatching: {:?}", path.replace("\\", "/"));
            }
            let _ = self.file_watcher.unwatch(path);
        }
        for path in self.new_watched_paths.difference(&self.watched_paths) {
            if let Some(path) = path.to_str() {
                trace!("Watching: {:?}", path.replace("\\", "/"));
            }
            let _ = self.file_watcher.watch(path, RecursiveMode::NonRecursive);
        }
        self.watched_paths = mem::take(&mut self.new_watched_paths);
    }

    pub fn get(&mut self, path: &str, store_content: bool) -> Result<FileCacheEntry> {
        let absolute_path = self.to_absolute_path(path);
        self.new_watched_paths.insert(absolute_path.clone());
        let result = match self.cache_map.entry(absolute_path.clone()) {
            Vacant(e) => {
                let source = std::fs::read(absolute_path)
                    .with_context(|| anyhow::format_err!("Failed to read file: '{path}'"))?;
                let hash = hash_content(&source);
                let entry = FileCacheEntry {
                    hash,
                    content: store_content.then(|| source.clone()),
                };
                e.insert(entry);
                FileCacheEntry {
                    hash,
                    content: Some(source),
                }
            }
            Occupied(e) => (*e.get()).clone(),
        };
        Ok(result)
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

pub fn hash_content(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
