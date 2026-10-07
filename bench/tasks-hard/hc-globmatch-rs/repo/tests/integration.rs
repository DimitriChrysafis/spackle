use globmatch::compile;

#[test]
fn doublestar_zero_dirs() {
    let m = compile("**/*.rs").unwrap();
    assert!(m.is_match("main.rs"), "should match zero dirs");
    assert!(m.is_match("a/main.rs"));
    assert!(m.is_match("a/b/main.rs"));
}

#[test]
fn doublestar_middle() {
    let m = compile("src/**/*.rs").unwrap();
    assert!(m.is_match("src/lib.rs"));
    assert!(m.is_match("src/a/b/lib.rs"));
    assert!(!m.is_match("other/lib.rs"));
}

#[test]
fn question_mark() {
    let m = compile("?.txt").unwrap();
    assert!(m.is_match("a.txt"));
    assert!(!m.is_match("ab.txt"));
}

#[test]
fn char_class() {
    let m = compile("[ab].txt").unwrap();
    assert!(m.is_match("a.txt"));
    assert!(m.is_match("b.txt"));
    assert!(!m.is_match("c.txt"));
}

#[test]
fn no_partial() {
    let m = compile("foo*").unwrap();
    assert!(m.is_match("foobar"));
    assert!(!m.is_match("xfoobar"));
}
