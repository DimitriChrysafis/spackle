//! Serialize an Ini back to text.

use crate::{Ini, Value};

pub fn to_string(ini: &Ini) -> String {
    let mut out = String::new();
    for section in ini.section_names() {
        if !section.is_empty() {
            out.push_str(&format!("[{section}]\n"));
        }
        if let Some(map) = ini.section(section) {
            for (k, v) in map {
                out.push_str(&format!("{k} = {}\n", render(v)));
            }
        }
    }
    out
}

fn render(v: &Value) -> String {
    match v {
        Value::Str(s) if s.contains(';') || s.contains('#') => {
            format!("\"{s}\"")
        }
        Value::Str(s) => s.clone(),
        Value::Num(n) => n.to_string(),
        Value::Flag(b) => b.to_string(),
    }
}
