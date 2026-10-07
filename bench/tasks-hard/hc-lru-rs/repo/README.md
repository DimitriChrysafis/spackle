# lru

Small LRU cache over `HashMap` + order list. `get`/`put` mark entries
most-recently-used; eviction drops the true LRU. `Counted` wraps it with
hit/miss accounting.

```rust
let mut c = Lru::new(100);
c.put("k", 1);
assert_eq!(c.get(&"k"), Some(&1));
```
