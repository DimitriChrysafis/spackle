    use lru::*;

    #[test]
    fn put_get() {
        let mut c = Lru::new(2);
        c.put("a", 1);
        assert_eq!(c.get(&"a"), Some(&1));
    }
