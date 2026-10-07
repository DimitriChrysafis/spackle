//! Ordered views over cache contents.

use crate::Lru;
use std::hash::Hash;

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    /// Keys from least- to most-recently-used.
    pub fn keys_oldest_first(&self) -> Vec<K> {
        self.keys_lru_order()
    }

    /// Iterate (key, is_most_recent) pairs in LRU order.
    pub fn ordered(&self) -> Vec<(K, bool)> {
        let n = self.order.len();
        self.order
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i + 1 == n))
            .collect()
    }
}
