use ahash::AHasher;
use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::mem;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

pub type FileContentHash = u64;

pub struct FileHashCache {
    cache_map: HashMap<PathBuf, FileContentHash>,
    watcher: RecommendedWatcher,
    file_change_events: Receiver<Result<notify::Event, notify::Error>>,

    // Load cycle state
    watched_paths: HashSet<PathBuf>,
    new_watched_paths: HashSet<PathBuf>,
}

impl FileHashCache {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        Self {
            cache_map: HashMap::new(),
            watcher,
            file_change_events: receiver,
            watched_paths: HashSet::new(),
            new_watched_paths: HashSet::new(),
        }
    }

    pub fn start_load_cycle(&mut self) -> bool {
        let mut has_changes = false;
        for res in self.file_change_events.try_iter() {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        println!("Removing file: {:?}", path);
                        self.cache_map.remove(&path);
                    }
                    has_changes = true;
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }
        has_changes
    }

    pub fn end_load_cycle(&mut self) -> Result<()> {
        for path in self.watched_paths.difference(&self.new_watched_paths) {
            self.watcher.unwatch(path)?;
        }
        for path in self.new_watched_paths.difference(&self.watched_paths) {
            self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        }
        self.watched_paths = mem::take(&mut self.new_watched_paths);
        Ok(())
    }

    pub fn get(&mut self, path: &PathBuf) -> Result<(FileContentHash, Option<Vec<u8>>)> {
        if let Some(hash) = self.cache_map.get(path) {
            Ok((*hash, None))
        } else {
            self.new_watched_paths.insert(path.clone());
            let source = std::fs::read(path)?;
            let hash = hash_content(&source);
            Ok((hash, Some(source)))
        }
    }
}

pub fn hash_content(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}
