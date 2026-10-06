pub fn fib(n: u32) -> u64 {
    let (mut a, mut b) = (0_u64, 1_u64);
    for _ in 0..n - 1 {
        let next = a + b;
        a = b;
        b = next;
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fib_seq() {
        assert_eq!(fib(0), 1);
        assert_eq!(fib(1), 1);
        assert_eq!(fib(2), 2);
        assert_eq!(fib(10), 89);
    }
}
