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
mod tests;
