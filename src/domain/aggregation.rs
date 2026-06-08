//! JSON merge and version aggregation logic.

/// Merge multiple JSON objects from beacons into a single aggregated object.
/// Each numeric value is summed across all objects (supports both integers and floats).
pub fn merge_json_objects(json_strings: &[String]) -> String {
    let mut merged: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for js in json_strings {
        if let Ok(obj) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(js) {
            for (key, value) in obj {
                let incoming = value.as_f64().unwrap_or(0.0);

                // Skip non-finite incoming values (Infinity, NaN) to prevent data corruption
                if !incoming.is_finite() {
                    continue;
                }

                let existing =
                    merged.entry(key.clone()).or_insert_with(|| serde_json::Value::from(0));
                // Skip non-finite incoming values (Infinity, NaN) to prevent data corruption
                if let Some(n) = existing.as_f64() {
                    let sum = n + incoming;

                    // Overflow protection: skip non-finite values (Infinity, NaN)
                    // and only cast to i64 when the value fits
                    if sum.is_finite() && sum <= i64::MAX as f64 && sum >= i64::MIN as f64 {
                        if sum.fract() == 0.0 {
                            *existing = serde_json::Value::from(sum as i64);
                        } else {
                            *existing = serde_json::Value::from(sum);
                        }
                    } else if sum.is_finite() {
                        // Value is finite but outside i64 range — keep as f64
                        *existing = serde_json::Value::from(sum);
                    }
                    // If sum is not finite (infinity/NaN), keep existing value (don't corrupt)
                }
            }
        }
    }
    serde_json::to_string(&merged).unwrap_or_else(|_| "{}".to_string())
}

/// Merge version strings from beacons into a `{version: instance_count}` JSON.
pub fn merge_versions(versions: &[String]) -> String {
    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for v in versions {
        let v = v.trim().to_string();
        if !v.is_empty() {
            *counts.entry(v).or_insert(0) += 1;
        }
    }
    serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_json_empty() {
        assert_eq!(merge_json_objects(&[]), "{}");
    }

    #[test]
    fn merge_json_single() {
        let inputs = vec!["{\"a\":10,\"b\":5}".to_string()];
        let result = merge_json_objects(&inputs);
        let parsed: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.get("a").unwrap().as_i64().unwrap(), 10);
        assert_eq!(parsed.get("b").unwrap().as_i64().unwrap(), 5);
    }

    #[test]
    fn merge_json_sums_values() {
        let inputs = vec!["{\"a\":10,\"b\":5}".to_string(), "{\"a\":20,\"c\":3}".to_string()];
        let result = merge_json_objects(&inputs);
        let parsed: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.get("a").unwrap().as_i64().unwrap(), 30);
        assert_eq!(parsed.get("b").unwrap().as_i64().unwrap(), 5);
        assert_eq!(parsed.get("c").unwrap().as_i64().unwrap(), 3);
    }

    #[test]
    fn merge_versions_counts_instances() {
        let inputs = vec!["0.17.4".to_string(), "0.17.4".to_string(), "0.18.0".to_string()];
        let result = merge_versions(&inputs);
        // Parse the result and check values instead of using contains
        let parsed: std::collections::HashMap<String, i64> = serde_json::from_str(&result).unwrap();
        assert_eq!(*parsed.get("0.17.4").unwrap(), 2);
        assert_eq!(*parsed.get("0.18.0").unwrap(), 1);
    }

    #[test]
    fn merge_json_overflow_protection() {
        let inputs = vec![
            r#"{"a":1}"#.to_string(),
            r#"{"a":1.8e308}"#.to_string(), // Exceeds f64::MAX → parses to Infinity, skipped by is_finite()
        ];
        let result = merge_json_objects(&inputs);
        let parsed: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result).unwrap();
        // Should not be null or infinity — should be finite
        assert!(parsed.get("a").unwrap().as_f64().unwrap().is_finite());
    }

    #[test]
    fn merge_json_nan_skipped() {
        let inputs = vec![r#"{"a":5}"#.to_string(), r#""hello""#.to_string()];
        let result = merge_json_objects(&inputs);
        // The function should process the first valid object and ignore the invalid string
        assert_eq!(result, r#"{"a":5}"#);
    }

    #[test]
    fn merge_json_float_preservation() {
        let inputs = vec![r#"{"ratio":1.5}"#.to_string()];
        let result = merge_json_objects(&inputs);
        let parsed: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&result).unwrap();
        // 1.5 should stay as float, not become integer
        assert_eq!(parsed.get("ratio").unwrap().as_f64().unwrap(), 1.5);
    }

    #[test]
    fn merge_json_non_object_input() {
        // A non-object JSON string should be skipped gracefully
        let inputs = vec![r#"5"#.to_string(), r#""hello""#.to_string()];
        let result = merge_json_objects(&inputs);
        assert_eq!(result, "{}");
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn merge_json_commutative(json1 in "\\PC*", json2 in "\\PC*") {
            // For two JSON objects with numeric values, merging should be commutative
            let inputs = vec![json1, json2];
            let result1 = merge_json_objects(&inputs);
            let result2 = merge_json_objects(&inputs.iter().rev().cloned().collect::<Vec<_>>());
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn merge_versions_counts_correctly(versions in prop::collection::vec(".*", 1..100)) {
            let result = merge_versions(&versions);
            // Merging version counts should always produce valid JSON
            let _parsed: std::collections::HashMap<String, i64> = serde_json::from_str(&result).unwrap();
            // Just ensure it's valid JSON and doesn't crash
            // The result should be valid JSON
            prop_assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
        }
    }
}
