use lru::Lru;

#[test]
fn evicts_oldest() {
    let mut c = Lru::new(2);
    c.put("a", 1);
    c.put("b", 2);
    c.put("c", 3); // evicts a
    assert_eq!(c.get(&"a"), None);
    assert_eq!(c.get(&"b"), Some(&2));
    assert_eq!(c.get(&"c"), Some(&3));
}

#[test]
fn get_refreshes_recency() {
    let mut c = Lru::new(2);
    c.put("a", 1);
    c.put("b", 2);
    let _ = c.get(&"a"); // touch a -> b is now oldest
    c.put("c", 3);
    assert_eq!(c.get(&"a"), Some(&1), "a was touched, should survive");
    assert_eq!(c.get(&"b"), None, "b is the real LRU");
}

#[test]
fn overwrite_keeps_position() {
    let mut c = Lru::new(2);
    c.put("a", 1);
    c.put("b", 2);
    c.put("a", 10);
    c.put("c", 3); // evicts b (a just rewritten)
    assert_eq!(c.get(&"a"), Some(&10));
    assert_eq!(c.get(&"b"), None);
}

#[test]
fn capacity_one() {
    let mut c = Lru::new(1);
    c.put("a", 1);
    c.put("b", 2);
    assert_eq!(c.get(&"a"), None);
    assert_eq!(c.get(&"b"), Some(&2));
}

#[test]
fn remove_works() {
    let mut c = Lru::new(3);
    c.put("a", 1);
    c.put("b", 2);
    assert_eq!(c.remove(&"b"), Some(2));
    assert_eq!(c.len(), 1);
}
