    use globmatch::*;

    #[test]
    fn literal() {
        let m = compile("src/main.rs").unwrap();
        assert!(m.is_match("src/main.rs"));
        assert!(!m.is_match("src/lib.rs"));
    }

    #[test]
    fn star() {
        let m = compile("*.txt").unwrap();
        assert!(m.is_match("a.txt"));
        assert!(m.is_match(".txt"));
        assert!(!m.is_match("a/b.txt"));
    }

    #[test]
    fn star_in_dir() {
        let m = compile("src/*.rs").unwrap();
        assert!(m.is_match("src/lib.rs"));
        assert!(!m.is_match("src/deep/lib.rs"));
    }
