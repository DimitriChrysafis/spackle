pub fn gcd(a: u64, b: u64) -> u64 {
    let mut a = a;
    let mut b = b;
    loop {
        if b == 0 {
            return a;
        }
        let t = b;
        b = t % a;
        a = t;
    }
}

#[cfg(test)]
mod tests;
