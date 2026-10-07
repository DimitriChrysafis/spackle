//! iniparse - classic INI files: [section], key = value, `;` and `#`
//! comments, quoted values, and line continuation via trailing `\`.

mod lexer;
mod parser;
pub mod write;

pub use parser::{parse_str, Ini, Value};
pub use write::to_string;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_and_key() {
        let ini = parse_str("[db]\nhost = localhost\nport = 5432\n").unwrap();
        assert_eq!(ini.get("db", "host"), Some(&Value::Str("localhost".into())));
    }
}
