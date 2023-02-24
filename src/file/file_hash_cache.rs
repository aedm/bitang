use ahash::AHasher;
use anyhow::{Context, Result};
use notify::RecommendedWatcher;
use std::collections::HashMap;
use std::hash::Hasher;
use std::mem;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

pub type FileContentHash = u64;

pub struct FileHashCache {
    cache_map: HashMap<PathBuf, FileContentHash>,
    watcher: RecommendedWatcher,
    content_change_receiver: Receiver<Result<notify::Event, notify::Error>>,

    // Load cycle state
    watched_paths: Vec<PathBuf>,
    new_watched_paths: Vec<PathBuf>,
}

impl FileHashCache {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(sender).unwrap();
        Self {
            cache_map: HashMap::new(),
            watcher,
            content_change_receiver: receiver,
            watched_paths: Vec::new(),
            new_watched_paths: Vec::new(),
        }
    }

    pub fn start_load_cycle(&mut self) -> bool {
        let mut has_changes = false;
        for res in self.content_change_receiver.try_iter() {
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
        self.new_watched_paths.clear();
        has_changes
    }

    pub fn end_load_cycle(&mut self, has_changes: bool) {
        // TODO: update watchers

        self.watched_paths = mem::take(&mut self.new_watched_paths);
    }

    pub fn get(&mut self, path: &PathBuf) -> Result<(FileContentHash, Option<Vec<u8>>)> {
        if let Some(hash) = self.cache_map.get(path) {
            Ok((*hash, None))
        } else {
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
