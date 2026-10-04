//! Text formatting helpers.

/// `12345` → `"12,345"` (`"12.345"` in Spanish).
pub fn thousands(n: u32) -> String {
    arclens_i18n::number(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
