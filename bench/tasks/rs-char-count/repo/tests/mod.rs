    use super::*;
    #[test]
    fn counts() {
        assert_eq!(count_char("banana", 'a'), 3);
        assert_eq!(count_char("banana", 'z'), 0);
    }
