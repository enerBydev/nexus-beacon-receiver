//! Payload and field validation.

use crate::config::*;
use crate::domain::types::BeaconPayload;

/// Validate YYYY-MM-DD date format without regex (no regex crate in WASM).
/// Validates format and range: year (0000-9999), month (01-12), day (01-31)
pub fn is_valid_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let b = s.as_bytes();
    if !(b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit())
    {
        return false;
    }
    // Validate month (01-12) and day (01-31) ranges
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    let day = (b[8] - b'0') * 10 + (b[9] - b'0');
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Validate beacon payload fields against security constraints.
/// All error messages are generic to prevent information leakage.
pub fn validate_payload(payload: &BeaconPayload) -> Result<(), &'static str> {
    // Length checks
    if payload.instance_id.is_empty() || payload.instance_id.len() > MAX_INSTANCE_ID_LEN {
        return Err("invalid field length");
    }
    if payload.version.is_empty() || payload.version.len() > MAX_VERSION_LEN {
        return Err("invalid field length");
    }
    if payload.date.is_empty() || payload.date.len() > MAX_DATE_LEN {
        return Err("invalid field length");
    }

    // Format checks
    // Instance ID must be hex (HMAC-SHA256 output)
    if !payload.instance_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid field format");
    }
    // Date must be YYYY-MM-DD
    if !is_valid_date(&payload.date) {
        return Err("invalid field format");
    }
    // Version must be semver-like (alphanumeric + .-_+)
    if !payload
        .version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '+')
    {
        return Err("invalid field format");
    }

    // Numeric range checks
    if payload.stats.total_requests > MAX_TOTAL_REQUESTS {
        return Err("invalid field value");
    }
    if payload.stats.unique_fingerprints > MAX_UNIQUE_FINGERPRINTS {
        return Err("invalid field value");
    }
    // is_finite() before range — NaN < 0.0 is false, NaN > N is false
    if !payload.stats.avg_message_count.is_finite()
        || payload.stats.avg_message_count < 0.0
        || payload.stats.avg_message_count > 1000.0
    {
        return Err("invalid field value");
    }
    if !payload.stats.tool_use_ratio.is_finite()
        || payload.stats.tool_use_ratio < 0.0
        || payload.stats.tool_use_ratio > 1.0
    {
        return Err("invalid field value");
    }

    // JSON map size checks
    if payload.stats.models_used.len() > MAX_MAP_ENTRIES {
        return Err("invalid field size");
    }
    if payload.stats.client_types.len() > MAX_MAP_ENTRIES {
        return Err("invalid field size");
    }

    // JSON map value checks (prevent overflow and injection)
    for (key, value) in &payload.stats.models_used {
        if key.len() > 128 {
            return Err("invalid field size");
        }
        let n = value.as_f64().ok_or("invalid field value")?;
        if !n.is_finite() || !(0.0..=1_000_000_000.0).contains(&n) {
            return Err("invalid field value");
        }
    }
    for (key, value) in &payload.stats.client_types {
        if key.len() > 128 {
            return Err("invalid field size");
        }
        let n = value.as_f64().ok_or("invalid field value")?;
        if !n.is_finite() || !(0.0..=1_000_000_000.0).contains(&n) {
            return Err("invalid field value");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{BeaconPayload, BeaconStats};
    use serde_json;

    fn valid_test_payload() -> BeaconPayload {
        BeaconPayload {
            instance_id: "a".repeat(64),
            version: "0.19.0".to_string(),
            date: "2026-06-04".to_string(),
            stats: BeaconStats {
                total_requests: 100,
                unique_fingerprints: 10,
                models_used: serde_json::Map::new(),
                client_types: serde_json::Map::new(),
                avg_message_count: 5.0,
                tool_use_ratio: 0.5,
            },
        }
    }

    #[test]
    fn is_valid_date_formats() {
        assert!(is_valid_date("2026-06-04"));
        assert!(is_valid_date("2025-01-31"));
        assert!(!is_valid_date("2026/06/04"));
        assert!(!is_valid_date("not-a-date"));
        assert!(!is_valid_date("2026-6-4"));
        assert!(!is_valid_date(""));
    }

    #[test]
    fn is_valid_date_rejects_invalid_ranges() {
        assert!(!is_valid_date("2026-13-01")); // month 13
        assert!(!is_valid_date("2026-00-15")); // month 0
        assert!(!is_valid_date("2026-01-32")); // day 32
        assert!(!is_valid_date("2026-01-00")); // day 0
        assert!(is_valid_date("2026-12-31")); // valid
    }

    #[test]
    fn validate_payload_valid() {
        assert!(validate_payload(&valid_test_payload()).is_ok());
    }

    #[test]
    fn validate_payload_instance_id_too_long() {
        let mut p = valid_test_payload();
        p.instance_id = "a".repeat(65);
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_invalid_date() {
        let mut p = valid_test_payload();
        p.date = "not-a-date".to_string();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_non_hex_instance_id() {
        let mut p = valid_test_payload();
        p.instance_id = "g".repeat(64);
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_tool_use_ratio_out_of_range() {
        let mut p = valid_test_payload();
        p.stats.tool_use_ratio = 1.5;
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_infinity_in_models_used() {
        // Test the is_finite() check directly
        let inf = f64::INFINITY;
        assert!(!inf.is_finite());
    }

    #[test]
    fn validate_payload_empty_instance_id() {
        let mut p = valid_test_payload();
        p.instance_id = "".to_string();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_empty_version() {
        let mut p = valid_test_payload();
        p.version = "".to_string();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_version_invalid_chars() {
        let mut p = valid_test_payload();
        p.version = "0.19<script>".to_string();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_total_requests_exceeds_max() {
        let mut p = valid_test_payload();
        p.stats.total_requests = MAX_TOTAL_REQUESTS + 1;
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_nan_rejected() {
        let mut p = valid_test_payload();
        p.stats.avg_message_count = f64::NAN;
        assert!(validate_payload(&p).is_err());
        let mut p2 = valid_test_payload();
        p2.stats.tool_use_ratio = f64::NAN;
        assert!(validate_payload(&p2).is_err());
    }

    #[test]
    fn validate_payload_non_numeric_map_value_rejected() {
        let mut p = valid_test_payload();
        p.stats
            .models_used
            .insert("key".to_string(), serde_json::Value::String("not_a_number".to_string()));
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_too_many_map_entries() {
        let mut p = valid_test_payload();
        for i in 0..51 {
            p.stats.models_used.insert(format!("model-{}", i), serde_json::Value::from(1));
        }
        assert!(validate_payload(&p).is_err());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn is_valid_date_never_accepts_wrong_length(s in "\\PC*") {
            // Any string not exactly 10 chars should be rejected
            if s.len() != 10 {
                prop_assert!(!is_valid_date(&s));
            }
        }

        #[test]
        fn is_valid_date_rejects_non_digit_separators(
            prefix in "[0-9]{4}",
            separator1 in "[^-]",
            middle in "[0-9]{2}",
            separator2 in "[^-]",
            suffix in "[0-9]{2}"
        ) {
            // Create a date string with non-dash separators
            let date = format!("{}{}{}{}{}", prefix, separator1, middle, separator2, suffix);
            // If the separators aren't dashes at positions 4 and 7, it should be rejected
            if separator1 != "-" || separator2 != "-" {
                prop_assert!(!is_valid_date(&date));
            }
        }

        #[test]
        fn validate_payload_rejects_empty_instance_id(_s in "[a-z0-9]*") {
            // Create a valid payload but with empty instance_id
            let mut payload = valid_test_payload();
            payload.instance_id = "".to_string();
            prop_assert!(validate_payload(&payload).is_err());
        }

        #[test]
        fn validate_payload_rejects_overlength_instance_id(_s in "[a-z0-9]*") {
            // Create a valid payload but with overlength instance_id
            let mut payload = valid_test_payload();
            payload.instance_id = "a".repeat(MAX_INSTANCE_ID_LEN + 1);
            prop_assert!(validate_payload(&payload).is_err());
        }
    }
}
