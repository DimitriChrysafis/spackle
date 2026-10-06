/// First index `i` where `xs[i] >= v` (insertion point for `v`).
pub fn insert_at(xs: &[i64], v: i64) -> usize {
    let (mut lo, mut hi) = (0_usize, xs.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if xs[mid] <= v {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

#[cfg(test)]
mod tests {
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
}
