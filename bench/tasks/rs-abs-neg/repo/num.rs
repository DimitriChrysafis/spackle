pub fn abs(x: i64) -> i64 {
    if x < 0 { x } else { -x }
}

#[cfg(test)]
mod tests;
