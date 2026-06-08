//! Security utilities (constant-time comparison, zeroize, auth).

/// Constant-time string comparison resistant to timing side-channel attacks.
/// XORs all bytes and ORs length difference so comparison time is independent
/// of where strings differ. Does NOT use ring/subtle (won't compile to wasm32).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result: u8 = 0;
    let max_len = a_bytes.len().max(b_bytes.len());
    for i in 0..max_len {
        let a_byte = a_bytes.get(i).copied().unwrap_or(0);
        let b_byte = b_bytes.get(i).copied().unwrap_or(0);
        result |= a_byte ^ b_byte;
    }
    result |= (a_bytes.len() != b_bytes.len()) as u8;
    result == 0
}

/// Case-insensitive "Bearer " prefix extraction from Authorization header.
/// Per RFC 7235, the auth-scheme token is case-insensitive.
pub fn extract_bearer_token(header: &str) -> &str {
    if header.len() >= 7 {
        let prefix = &header[..7];
        if prefix.eq_ignore_ascii_case("Bearer ") {
            return &header[7..];
        }
    }
    header
}

/// Overwrite a String's heap memory with zeroes to prevent credential leakage
/// after comparison. Uses volatile writes to prevent compiler optimization from
/// eliding the zeroing. The String is then cleared to length 0.
pub fn zeroize_string(s: &mut String) {
    let bytes = unsafe { s.as_bytes_mut() };
    for byte in bytes.iter_mut() {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    s.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_equal_strings() {
        assert!(constant_time_eq("abc123", "abc123"));
    }

    #[test]
    fn constant_time_eq_different_strings() {
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc1234"));
        assert!(!constant_time_eq("short", "much_longer_string"));
    }

    #[test]
    fn constant_time_eq_empty_strings() {
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("", "a"));
    }

    #[test]
    fn constant_time_eq_single_char() {
        assert!(constant_time_eq("a", "a"));
        assert!(!constant_time_eq("a", "b"));
    }

    #[test]
    fn constant_time_eq_unicode() {
        assert!(constant_time_eq("café", "café"));
        assert!(!constant_time_eq("café", "cafe"));
    }

    #[test]
    fn extract_bearer_token_standard() {
        assert_eq!(extract_bearer_token("Bearer abc123"), "abc123");
    }

    #[test]
    fn extract_bearer_token_lowercase() {
        assert_eq!(extract_bearer_token("bearer abc123"), "abc123");
    }

    #[test]
    fn extract_bearer_token_mixed_case() {
        assert_eq!(extract_bearer_token("BEARER abc123"), "abc123");
    }

    #[test]
    fn extract_bearer_token_no_prefix() {
        assert_eq!(extract_bearer_token("abc123"), "abc123");
    }

    #[test]
    fn extract_bearer_token_empty_after_prefix() {
        assert_eq!(extract_bearer_token("Bearer "), "");
    }

    #[test]
    fn extract_bearer_token_tab_separator() {
        // Tab is NOT a space — "Bearer\t" should NOT match
        assert_eq!(extract_bearer_token("Bearer\tabc"), "Bearer\tabc");
    }

    #[test]
    fn zeroize_string_clears_content() {
        let mut s = String::from("secret_token_value");
        zeroize_string(&mut s);
        assert_eq!(s.len(), 0);
        assert_eq!(s, "");
    }

    #[test]
    fn zeroize_string_empty() {
        let mut s = String::new();
        zeroize_string(&mut s);
        assert_eq!(s, "");
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn constant_time_eq_reflexive(a in ".*") {
            // For any string `a`, `constant_time_eq(a, a)` is true
            prop_assert!(constant_time_eq(&a, &a));
        }

        #[test]
        fn constant_time_eq_symmetric(a in ".*", b in ".*") {
            // For any strings `a, b`, `constant_time_eq(a, b) == constant_time_eq(b, a)`
            let result1 = constant_time_eq(&a, &b);
            let result2 = constant_time_eq(&b, &a);
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn constant_time_eq_negation(a in ".*", b in ".*") {
            // For any strings `a, b` where `a != b`, `constant_time_eq(a, b)` is false
            if a != b {
                prop_assert!(!constant_time_eq(&a, &b));
            }
        }
    }
}
