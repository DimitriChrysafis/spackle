    use iniparse::*;

    #[test]
    fn section_and_key() {
        let ini = parse_str("[db]\nhost = localhost\nport = 5432\n").unwrap();
        assert_eq!(ini.get("db", "host"), Some(&Value::Str("localhost".into())));
    }
