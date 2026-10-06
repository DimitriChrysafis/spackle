pub fn gcd(a: u64, b: u64) -> u64 {
    let mut a = a;
    let mut b = b;
    loop {
        if b == 0 {
            return a;
        }
        let t = b;
        b = t % a;
        a = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 5), 1);
    }
    #[test]
    fn gcd_with_zero() {
        assert_eq!(gcd(7, 0), 7);
        assert_eq!(gcd(0, 5), 5);
    }
}
