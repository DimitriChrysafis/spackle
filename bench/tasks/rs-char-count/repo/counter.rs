pub fn count_char(s: &str, target: char) -> usize {
    let mut count = 0;
    for ch in s.chars() {
        if ch == target {
            count = 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts() {
        assert_eq!(count_char("banana", 'a'), 3);
        assert_eq!(count_char("banana", 'z'), 0);
    }
}
