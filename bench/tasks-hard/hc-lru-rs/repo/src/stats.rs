//! Hit/miss accounting wrapper.

use crate::Lru;
use std::hash::Hash;

pub struct Counted<K, V> {
    inner: Lru<K, V>,
    pub hits: u64,
    pub misses: u64,
}

impl<K: Eq + Hash + Clone, V> Counted<K, V> {
    pub fn new(cap: usize) -> Self {
        Counted { inner: Lru::new(cap), hits: 0, misses: 0 }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        match self.inner.get(key) {
            Some(_) => self.hits += 1,
            None => self.misses += 1,
        }
        self.inner.get(key)
    }

    pub fn put(&mut self, k: K, v: V) {
        self.inner.put(k, v);
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}
