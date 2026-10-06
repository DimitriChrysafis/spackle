pub fn abs(x: i64) -> i64 {
    if x < 0 { x } else { -x }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abs_works() {
        assert_eq!(abs(-5), 5);
        assert_eq!(abs(3), 3);
        assert_eq!(abs(0), 0);
    }
}
