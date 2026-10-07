    use super::*;
    #[test]
    fn fib_seq() {
        assert_eq!(fib(0), 1);
        assert_eq!(fib(1), 1);
        assert_eq!(fib(2), 2);
        assert_eq!(fib(10), 89);
    }
