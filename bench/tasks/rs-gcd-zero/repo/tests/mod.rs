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
