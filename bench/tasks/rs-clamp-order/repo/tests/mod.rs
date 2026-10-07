    use super::*;
    #[test]
    fn clamps() {
        assert_eq!(clamp(15, 0, 10), 10);
        assert_eq!(clamp(-3, 0, 10), 0);
        assert_eq!(clamp(5, 0, 10), 5);
    }
