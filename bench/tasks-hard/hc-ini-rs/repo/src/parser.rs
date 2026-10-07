//! Logical lines -> Ini document.

use crate::lexer::{logical_lines, Line};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(i64),
    Flag(bool),
}

#[derive(Debug, Default)]
pub struct Ini {
    sections: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Ini {
    pub fn get(&self, section: &str, key: &str) -> Option<&Value> {
        self.sections.get(section)?.get(key)
    }

    pub fn section(&self, name: &str) -> Option<&BTreeMap<String, Value>> {
        self.sections.get(name)
    }

    pub fn section_names(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }
}

pub fn parse_str(text: &str) -> Result<Ini, String> {
    let mut ini = Ini::default();
    let mut cur = String::new();
    ini.sections.insert(cur.clone(), BTreeMap::new());
    for (lineno, line) in logical_lines(text) {
        match line {
            Line::Blank => {}
            Line::Section(name) => {
                cur = name;
                ini.sections.entry(cur.clone()).or_default();
            }
            Line::Assign { key, raw } => {
                let value = decode(&raw)
                    .map_err(|e| format!("line {lineno}: {e}"))?;
                ini.sections.get_mut(&cur).unwrap().insert(key, value);
            }
        }
    }
    Ok(ini)
}

fn decode(raw: &str) -> Result<Value, String> {
    if raw.starts_with('"') {
        if raw.len() < 2 || !raw.ends_with('"') {
            return Err("unterminated quoted value".into());
        }
        return Ok(Value::Str(raw[1..raw.len() - 1].to_string()));
    }
    match raw {
        "true" | "yes" | "on" => return Ok(Value::Flag(true)),
        "false" | "no" | "off" => return Ok(Value::Flag(false)),
        _ => {}
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Ok(Value::Num(n));
    }
    Ok(Value::Str(raw.to_string()))
}
