pub fn median(xs: &[f64]) -> f64 {
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[sorted.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn odd() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
    }
    #[test]
    fn even() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }
}
