    use super::*;
    #[test]
    fn odd() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
    }
    #[test]
    fn even() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }
