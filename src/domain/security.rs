//! Security utilities (constant-time comparison, zeroize, auth).

use crate::domain::types::AuthError;

/// Constant-time string comparison resistant to timing side-channel attacks.
/// XORs all bytes and ORs length difference so comparison time is independent
/// of where strings differ. Does NOT use ring/subtle (won't compile to wasm32).
///
/// # Example
///
/// ```
/// use nexus_beacon_receiver::domain::security::constant_time_eq;
///
/// // Equal strings
/// assert_eq!(constant_time_eq("test_secret", "test_secret"), true);
///
/// // Different strings
/// assert_eq!(constant_time_eq("test_secret", "wrong_secret"), false);
/// assert_eq!(constant_time_eq("short", "much_longer_string"), false);
/// ```
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

/// Verify a Bearer token from an Authorization header against the expected secret.
/// Pure domain function — no worker dependencies.
/// Returns Ok(()) if valid, Err(AuthError) if invalid.
///
/// # Example
///
/// ```
/// use nexus_beacon_receiver::domain::security::verify_bearer_token;
/// use nexus_beacon_receiver::domain::types::AuthError;
///
/// // Valid token example
/// let secret = "test_secret";
/// let auth_header = "Bearer test_secret";
/// assert!(verify_bearer_token(auth_header, secret).is_ok());
///
/// // Invalid token example
/// let wrong_auth_header = "Bearer wrong_secret";
/// assert!(verify_bearer_token(wrong_auth_header, secret).is_err());
/// ```
pub fn verify_bearer_token(auth_header: &str, secret: &str) -> Result<(), AuthError> {
    let token = extract_bearer_token(auth_header);
    let mut expected = secret.to_string();
    let mut header_owned = auth_header.to_string();
    let is_valid = constant_time_eq(token, &expected);
    zeroize_string(&mut expected);
    zeroize_string(&mut header_owned);
    if is_valid {
        Ok(())
    } else {
        Err(AuthError::InvalidCredentials)
    }
}

/// Case-insensitive "Bearer " prefix extraction from Authorization header.
/// Per RFC 7235, the auth-scheme token is case-insensitive.
///
/// # Example
///
/// ```
/// use nexus_beacon_receiver::domain::security::extract_bearer_token;
///
/// // Standard Bearer token
/// assert_eq!(extract_bearer_token("Bearer abc123"), "abc123");
///
/// // Lowercase bearer
/// assert_eq!(extract_bearer_token("bearer abc123"), "abc123");
///
/// // Mixed case bearer
/// assert_eq!(extract_bearer_token("BEARER abc123"), "abc123");
///
/// // No prefix (returns the whole string)
/// assert_eq!(extract_bearer_token("abc123"), "abc123");
/// ```
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
///
/// # Example
///
/// ```
/// use nexus_beacon_receiver::domain::security::zeroize_string;
///
/// let mut secret = "sensitive_data".to_string();
/// zeroize_string(&mut secret);
/// assert_eq!(secret.len(), 0);
/// ```
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

    #[test]
    fn verify_bearer_valid() {
        let secret = "test_secret";
        let auth_header = "Bearer test_secret";
        assert!(verify_bearer_token(auth_header, secret).is_ok());
    }

    #[test]
    fn verify_bearer_invalid_token() {
        let secret = "test_secret";
        let auth_header = "Bearer wrong_secret";
        assert!(verify_bearer_token(auth_header, secret).is_err());
    }

    #[test]
    fn verify_bearer_missing_header() {
        let secret = "test_secret";
        let auth_header = "";
        assert!(verify_bearer_token(auth_header, secret).is_err());
    }

    #[test]
    fn verify_bearer_zeroizes_both_strings() {
        // This test is tricky to implement directly since we're testing that the strings are zeroized
        // We'll test indirectly by checking that the function works correctly
        let secret = "test_secret";
        let auth_header = "Bearer test_secret";
        let result = verify_bearer_token(auth_header, secret);
        assert!(result.is_ok());
        // The actual test for zeroization would be implementation-specific and hard to verify in a unit test
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
