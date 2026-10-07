//! Physical lines -> logical lines: join `\`-continuations, strip
//! comments, classify header vs assignment.

#[derive(Debug, PartialEq)]
pub enum Line {
    Section(String),
    Assign { key: String, raw: String },
    Blank,
}

pub fn logical_lines(text: &str) -> Vec<(usize, Line)> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut start_line = 0usize;
    for (idx, phys) in text.lines().enumerate() {
        let mut l = phys.to_string();
        if buf.is_empty() {
            start_line = idx + 1;
        }
        if l.ends_with('\\') {
            l.pop();
            buf.push_str(&l);
            continue;
        }
        buf.push_str(&l);
        let logical = buf.clone();
        buf.clear();
        out.push((start_line, classify(&logical)));
    }
    if !buf.is_empty() {
        out.push((start_line, classify(&buf)));
    }
    out
}

fn classify(line: &str) -> Line {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
        return Line::Blank;
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Line::Section(trimmed[1..trimmed.len() - 1].trim().to_string());
    }
    if let Some(eq) = trimmed.find('=') {
        let key = trimmed[..eq].trim().to_string();
        let raw = strip_comment(trimmed[eq + 1..].trim());
        return Line::Assign { key, raw };
    }
    Line::Assign { key: trimmed.to_string(), raw: String::new() }
}

fn strip_comment(v: &str) -> String {
    match v.find(';').or_else(|| v.find('#')) {
        Some(i) => v[..i].trim_end().to_string(),
        None => v.to_string(),
    }
}
