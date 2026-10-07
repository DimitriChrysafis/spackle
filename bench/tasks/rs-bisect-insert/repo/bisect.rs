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
mod tests;
