pub fn median(xs: &[f64]) -> f64 {
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[sorted.len() / 2]
}

#[cfg(test)]
mod tests;
