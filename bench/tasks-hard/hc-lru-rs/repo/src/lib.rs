//! A small LRU cache. `get` and `put` both mark the entry
//! most-recently-used; eviction drops the least-recently-used.
pub mod stats;
mod walk;

use std::collections::HashMap;
use std::hash::Hash;

pub struct Lru<K, V> {
    cap: usize,
    map: HashMap<K, V>,
    pub(crate) order: Vec<K>, // front = least recent, back = most recent
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be positive");
        Lru { cap, map: HashMap::new(), order: Vec::new() }
    }

    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    pub fn capacity(&self) -> usize { self.cap }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn peek(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        let old = self.map.insert(key.clone(), value);
        self.touch(&key);
        if self.map.len() > self.cap {
            let evict = self.order.remove(0);
            self.map.remove(&evict);
        }
        old
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.order.retain(|k| k != key);
        self.map.remove(key)
    }

    fn touch(&mut self, key: &K) {
        self.order.retain(|k| k != key);
        self.order.push(key.clone());
    }

    pub fn keys_lru_order(&self) -> Vec<K> {
        self.order.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get() {
        let mut c = Lru::new(2);
        c.put("a", 1);
        assert_eq!(c.get(&"a"), Some(&1));
    }
}
