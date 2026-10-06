pub fn escape(s: &str) -> String {
    s.replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('&', "&amp;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        assert_eq!(escape("<b>"), "&lt;b&gt;");
    }
    #[test]
    fn ampersand() {
        assert_eq!(escape("&"), "&amp;");
        assert_eq!(escape("<b>&amp;</b>"), "&lt;b&gt;&amp;amp;&lt;/b&gt;");
    }
}
