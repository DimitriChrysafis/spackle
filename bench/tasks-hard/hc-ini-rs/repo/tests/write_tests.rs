use iniparse::{parse_str, to_string};

#[test]
fn round_trip() {
    let src = "[s]\nx = 1\ny = hello\n";
    let ini = parse_str(src).unwrap();
    let out = to_string(&ini);
    let again = parse_str(&out).unwrap();
    assert_eq!(again.get("s", "x"), ini.get("s", "x"));
}
