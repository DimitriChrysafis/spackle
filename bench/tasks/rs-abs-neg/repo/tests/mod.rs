    use super::*;
    #[test]
    fn abs_works() {
        assert_eq!(abs(-5), 5);
        assert_eq!(abs(3), 3);
        assert_eq!(abs(0), 0);
    }
