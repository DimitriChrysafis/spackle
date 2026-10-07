pub fn escape(s: &str) -> String {
    s.replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('&', "&amp;")
}

#[cfg(test)]
mod tests;
