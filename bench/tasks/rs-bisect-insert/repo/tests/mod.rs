    use super::*;
    #[test]
    fn inserts() {
        assert_eq!(insert_at(&[1, 3, 5], 4), 2);
        assert_eq!(insert_at(&[1, 3, 5], 6), 3);
        assert_eq!(insert_at(&[1, 3, 5], 0), 0);
        assert_eq!(insert_at(&[1, 3, 5], 1), 0);
    }
    #[test]
    fn empty() {
        assert_eq!(insert_at(&[], 5), 0);
    }
