use iniparse::{parse_str, Value};

#[test]
fn quoted_semicolon_value() {
    let ini = parse_str("[x]\nmsg = \"a; b\"\n").unwrap();
    assert_eq!(ini.get("x", "msg"), Some(&Value::Str("a; b".into())));
}

#[test]
fn quoted_hash_value() {
    let ini = parse_str("[x]\nmsg = \"a # b\"\n").unwrap();
    assert_eq!(ini.get("x", "msg"), Some(&Value::Str("a # b".into())));
}

#[test]
fn comment_after_value() {
    let ini = parse_str("[x]\nn = 1 ; trailing\n").unwrap();
    assert_eq!(ini.get("x", "n"), Some(&Value::Num(1)));
}

#[test]
fn flags_and_nums() {
    let ini = parse_str("[f]\non = true\noff = no\nn = -3\n").unwrap();
    assert_eq!(ini.get("f", "on"), Some(&Value::Flag(true)));
    assert_eq!(ini.get("f", "off"), Some(&Value::Flag(false)));
    assert_eq!(ini.get("f", "n"), Some(&Value::Num(-3)));
}

#[test]
fn continuation() {
    let ini = parse_str("[x]\nval = hello \\\n  world\n").unwrap();
    assert_eq!(ini.get("x", "val"), Some(&Value::Str("hello   world".into())));
}

#[test]
fn global_section() {
    let ini = parse_str("top = 1\n[s]\nx = 2\n").unwrap();
    assert_eq!(ini.get("", "top"), Some(&Value::Num(1)));
}

#[test]
fn multiple_sections() {
    let ini = parse_str("[a]\nx=1\n[b]\ny=2\n").unwrap();
    assert_eq!(ini.section_names(), vec!["", "a", "b"]);
}
