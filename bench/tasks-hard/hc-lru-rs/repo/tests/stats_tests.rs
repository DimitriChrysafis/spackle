use lru::stats::Counted;

#[test]
fn counts_hits_and_misses() {
    let mut c = Counted::new(2);
    c.put("a", 1);
    let _ = c.get(&"a");
    let _ = c.get(&"b");
    assert_eq!(c.hits, 1);
    assert_eq!(c.misses, 1);
    assert!((c.hit_rate() - 0.5).abs() < 1e-9);
}
