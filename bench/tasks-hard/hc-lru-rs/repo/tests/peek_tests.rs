use lru::Lru;

#[test]
fn peek_does_not_refresh() {
    let mut c = Lru::new(2);
    c.put("a", 1);
    c.put("b", 2);
    let _ = c.peek(&"a"); // peek must NOT touch
    c.put("c", 3);
    assert_eq!(c.get(&"a"), None, "peek leaves a as LRU");
}
