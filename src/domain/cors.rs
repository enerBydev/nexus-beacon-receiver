//! CORS origin parsing — pure domain logic.

/// Parse a comma-separated CORS origins string into a Vec of trimmed, non-empty origins.
/// Pure function — no worker dependencies.
///
/// # Example
///
/// ```
/// use nexus_beacon_receiver::domain::cors::parse_cors_origins;
///
/// let origins = "https://example.com, https://test.com";
/// let result = parse_cors_origins(origins);
/// assert_eq!(result, vec!["https://example.com".to_string(), "https://test.com".to_string()]);
///
/// let empty_origins = "";
/// let result = parse_cors_origins(empty_origins);
/// assert_eq!(result, Vec::<String>::new());
/// ```
pub fn parse_cors_origins(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cors_splits_and_trims() {
        let result = parse_cors_origins("a.com, b.com");
        assert_eq!(result, vec!["a.com".to_string(), "b.com".to_string()]);
    }

    #[test]
    fn parse_cors_ignores_empty() {
        let result = parse_cors_origins("a.com,,b.com");
        assert_eq!(result, vec!["a.com".to_string(), "b.com".to_string()]);
    }

    #[test]
    fn parse_cors_no_wildcard_in_default() {
        // The default fallback string doesn't contain "*"
        let default = "https://enerby.dev,https://www.enerby.dev";
        let origins = parse_cors_origins(default);
        assert!(!origins.contains(&"*".to_string()));
        assert!(origins.contains(&"https://enerby.dev".to_string()));
    }

    #[test]
    fn parse_cors_empty_string() {
        let result = parse_cors_origins("");
        assert_eq!(result, Vec::<String>::new());
    }
}
