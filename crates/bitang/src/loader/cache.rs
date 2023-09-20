use ahash::AHashMap;
use anyhow::Result;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::hash::Hash;

/// A simple cache that stores the result of a fallible function call.
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
        &self,
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
