//! iniparse - classic INI files: [section], key = value, `;` and `#`
//! comments, quoted values, and line continuation via trailing `\`.

mod lexer;
mod parser;
pub mod write;

pub use parser::{parse_str, Ini, Value};
pub use write::to_string;
