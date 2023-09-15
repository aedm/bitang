use ahash::{AHashMap, AHasher};
use anyhow::Result;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::hash::{Hash, Hasher};

pub mod file_cache;
pub mod resource_cache;
pub mod resource_repository;
pub mod shader_loader;

pub fn compute_hash(content: &[u8]) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(content);
    hasher.finish()
}

pub struct Cache<Key: Hash + Eq + Clone, Value> {
    items: AHashMap<Key, Value>,
}

impl<Key: Hash + Eq + Clone, Value> Cache<Key, Value> {
    pub fn new() -> Self {
        Self {
            items: AHashMap::new(),
        }
    }

    pub fn get_or_try_insert_with_key<F: FnOnce(&Key) -> Result<Value>>(
        &mut self,
        key: Key,
        loader: F,
    ) -> Result<&Value> {
        let value_ref = match self.items.entry(key) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => {
                let value = loader(entry.key())?;
                entry.insert(value)
            }
        };
        Ok(value_ref)
    }

    pub fn remove(&mut self, key: &Key) {
        self.items.remove(key);
    }
}
