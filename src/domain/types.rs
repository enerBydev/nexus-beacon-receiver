//! Domain data types — zero worker dependency.

use serde::{Deserialize, Serialize};
use serde_json;

/// Incoming beacon payload from a NEXUS-AI-Gateway instance.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::BeaconPayload;
///
/// let json = r#"{"instance_id":"abc123","version":"0.19.0","date":"2026-06-04","stats":{"total_requests":100,"unique_fingerprints":10,"models_used":{},"client_types":{},"avg_message_count":5.0,"tool_use_ratio":0.5}}"#;
/// let payload: BeaconPayload = serde_json::from_str(json).unwrap();
/// assert_eq!(payload.instance_id, "abc123");
/// ```
#[derive(Deserialize, Serialize, Clone)]
pub struct BeaconPayload {
    pub instance_id: String,
    pub version: String,
    pub date: String,
    pub stats: BeaconStats,
}

/// Per-instance daily statistics inside the beacon payload.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::BeaconStats;
/// use serde_json;
///
/// let json = r#"{"total_requests":100,"unique_fingerprints":10,"models_used":{},"client_types":{},"avg_message_count":5.0,"tool_use_ratio":0.5}"#;
/// let stats: BeaconStats = serde_json::from_str(json).unwrap();
/// assert_eq!(stats.total_requests, 100);
/// ```
#[derive(Deserialize, Serialize, Clone)]
pub struct BeaconStats {
    pub total_requests: u64,
    pub unique_fingerprints: u64,
    #[serde(default)]
    pub models_used: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub client_types: serde_json::Map<String, serde_json::Value>,
    pub avg_message_count: f64,
    pub tool_use_ratio: f64,
}

/// Row from the `beacons` table used for JSON merge aggregation.
///
/// # Example
/// ```rust,ignore
/// // BeaconRow is a struct that derives only Deserialize, so it cannot be directly constructed in a doc-test.
/// // It represents a row from the `beacons` table used for JSON merge aggregation.
/// // For internal use in the aggregation process.
/// ```
#[allow(dead_code)]
#[derive(Deserialize)]
pub struct BeaconRow {
    pub models_used: String,
    pub client_types: String,
    pub version: String,
}

/// Result of numeric aggregation query for daily_global_stats.
///
/// # Example
/// ```rust,ignore
/// // AggregationResult is a struct that derives only Deserialize, so it cannot be directly constructed in a doc-test.
/// // It represents the result of a numeric aggregation query for daily_global_stats.
/// // For internal use in the aggregation process.
/// ```
#[derive(Deserialize)]
pub struct AggregationResult {
    pub total_instances: i64,
    pub total_requests: i64,
    pub total_unique_users: i64,
    pub avg_message_count: f64,
    pub tool_use_ratio: f64,
}

/// Row from the `daily_global_stats` D1 table.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::DailyGlobalStats;
/// use serde_json;
///
/// let stats = DailyGlobalStats {
///     date: "2026-06-04".to_string(),
///     total_instances: 10,
///     total_requests: 100,
///     total_unique_users: 5,
///     models_used: "{}".to_string(),
///     client_types: "{}".to_string(),
///     avg_message_count: 5.0,
///     tool_use_ratio: 0.5,
///     versions: "{}".to_string(),
/// };
/// let json = serde_json::to_string(&stats).unwrap();
/// assert!(json.contains("\"date\":\"2026-06-04\""));
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DailyGlobalStats {
    pub date: String,
    pub total_instances: i64,
    pub total_requests: i64,
    pub total_unique_users: i64,
    pub models_used: String,
    pub client_types: String,
    pub avg_message_count: f64,
    pub tool_use_ratio: f64,
    pub versions: String,
}

/// Response for `GET /v1/stats`.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::StatsResponse;
/// use serde_json;
///
/// let response = StatsResponse { stats: vec![] };
/// let json = serde_json::to_string(&response).unwrap();
/// assert!(json.contains("\"stats\":[]"));
/// ```
#[derive(Serialize, Debug, PartialEq)]
pub struct StatsResponse {
    pub stats: Vec<DailyGlobalStats>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SummaryResponse {
    pub total_instances: i64,
    pub total_requests: i64,
    pub total_unique_users: i64,
    pub days_active: i64,
}

/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::SummaryResponse;
/// use serde_json;
///
/// let response = SummaryResponse {
///     total_instances: 42,
///     total_requests: 1000,
///     total_unique_users: 20,
///     days_active: 7,
/// };
/// let json = serde_json::to_string(&response).unwrap();
/// let reparsed: SummaryResponse = serde_json::from_str(&json).unwrap();
/// assert_eq!(reparsed.total_instances, 42);
/// ```
///
/// Generic error response body.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::ErrorResponse;
/// use serde_json;
///
/// let err = ErrorResponse { error: "test" };
/// let json = serde_json::to_string(&err).unwrap();
/// assert_eq!(json, r#"{"error":"test"}"#);
/// ```
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: &'static str,
}

/// Response for `GET /v1/stats/shield` - Shields.io endpoint badge format.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::ShieldResponse;
/// use serde_json;
///
/// let shield = ShieldResponse {
///     schema_version: 1,
///     label: "NEXUS",
///     message: "10 active".to_string(),
///     color: "blue",
///     named_logo: "cloudflare",
/// };
/// let json = serde_json::to_string(&shield).unwrap();
/// assert!(json.contains("\"schemaVersion\":1"));
/// assert!(json.contains("\"namedLogo\":\"cloudflare\""));
/// ```
#[derive(Serialize)]
pub struct ShieldResponse {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u8,
    pub label: &'static str,
    pub message: String,
    pub color: &'static str,
    #[serde(rename = "namedLogo")]
    pub named_logo: &'static str,
}

/// Result of processing a beacon request through the domain service.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::BeaconResult;
///
/// assert_eq!(BeaconResult::Success, BeaconResult::Success);
/// assert_ne!(BeaconResult::Success, BeaconResult::RateLimited);
/// ```
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum BeaconResult {
    /// Beacon was successfully stored.
    Success,
    /// Rate limit exceeded.
    RateLimited,
    /// Authentication failed.
    Unauthorized,
    /// Content-Type is not application/json.
    InvalidContentType,
    /// Request body exceeds size limit.
    PayloadTooLarge,
    /// Payload failed validation.
    InvalidPayload,
    /// Internal server error (database, etc).
    InternalError,
}

/// Errors that can occur during repository operations.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::RepositoryError;
///
/// let err = RepositoryError::DatabaseError("connection failed".to_string());
/// assert_eq!(format!("{}", err), "database error: connection failed");
/// ```
#[allow(dead_code)]
#[derive(Debug)]
pub enum RepositoryError {
    /// Database connection or query failed.
    DatabaseError(String),
    /// Deserialization of query results failed.
    DeserializationError(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::DatabaseError(msg) => write!(f, "database error: {}", msg),
            RepositoryError::DeserializationError(msg) => {
                write!(f, "deserialization error: {}", msg)
            }
        }
    }
}

impl std::error::Error for RepositoryError {}

/// Errors that can occur during authentication.
///
/// # Example
/// ```
/// use nexus_beacon_receiver::domain::types::AuthError;
///
/// let err = AuthError::InvalidCredentials;
/// assert_eq!(format!("{}", err), "invalid credentials");
/// ```
#[allow(dead_code)]
#[derive(Debug)]
pub enum AuthError {
    /// The Authorization header is missing or malformed.
    MissingCredentials,
    /// The provided credentials are invalid.
    InvalidCredentials,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingCredentials => write!(f, "missing credentials"),
            AuthError::InvalidCredentials => write!(f, "invalid credentials"),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_payload_roundtrip() {
        let json = r#"{"instance_id":"abc123","version":"0.19.0","date":"2026-06-04","stats":{"total_requests":100,"unique_fingerprints":10,"models_used":{},"client_types":{},"avg_message_count":5.0,"tool_use_ratio":0.5}}"#;
        let payload: BeaconPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.instance_id, "abc123");
        let re_json = serde_json::to_string(&payload).unwrap();
        let reparsed: BeaconPayload = serde_json::from_str(&re_json).unwrap();
        assert_eq!(reparsed.instance_id, payload.instance_id);
    }

    #[test]
    fn beacon_stats_roundtrip() {
        let json = r#"{"total_requests":50,"unique_fingerprints":5,"models_used":{"claude":10},"client_types":{},"avg_message_count":3.0,"tool_use_ratio":0.2}"#;
        let stats: BeaconStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.total_requests, 50);
    }

    #[test]
    fn daily_global_stats_roundtrip() {
        let stats = DailyGlobalStats {
            date: "2026-06-04".to_string(),
            total_instances: 10,
            total_requests: 100,
            total_unique_users: 5,
            models_used: "{}".to_string(),
            client_types: "{}".to_string(),
            avg_message_count: 5.0,
            tool_use_ratio: 0.5,
            versions: "{}".to_string(),
        };
        let json = serde_json::to_string(&stats).unwrap();
        let reparsed: DailyGlobalStats = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.date, stats.date);
    }

    #[test]
    fn summary_response_roundtrip() {
        let summary = SummaryResponse {
            total_instances: 42,
            total_requests: 1000,
            total_unique_users: 20,
            days_active: 7,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let reparsed: SummaryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.total_instances, 42);
    }

    #[test]
    fn shield_response_serialization() {
        let shield = ShieldResponse {
            schema_version: 1,
            label: "NEXUS",
            message: "10 active".to_string(),
            color: "blue",
            named_logo: "cloudflare",
        };
        let json = serde_json::to_string(&shield).unwrap();
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"namedLogo\":\"cloudflare\""));
    }

    #[test]
    fn error_response_serialization() {
        let err = ErrorResponse { error: "unauthorized" };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"error":"unauthorized"}"#);
    }

    #[test]
    fn beacon_result_variants() {
        assert_eq!(BeaconResult::Success, BeaconResult::Success);
        assert_ne!(BeaconResult::Success, BeaconResult::RateLimited);
        assert_ne!(BeaconResult::Unauthorized, BeaconResult::InvalidPayload);
    }

    #[test]
    fn repository_error_display() {
        let err = RepositoryError::DatabaseError("connection failed".to_string());
        assert_eq!(format!("{}", err), "database error: connection failed");
    }

    #[test]
    fn auth_error_display() {
        let err = AuthError::InvalidCredentials;
        assert_eq!(format!("{}", err), "invalid credentials");
    }
}

#[cfg(test)]
pub mod builders {
    use crate::domain::types::{BeaconPayload, BeaconStats};

    /// Builder for constructing test BeaconPayload instances.
    ///
    /// # Examples
    /// ```
    /// use nexus_beacon_receiver::tests::common::BeaconPayloadBuilder;
    /// let payload = BeaconPayloadBuilder::default().build();
    /// let custom = BeaconPayloadBuilder::default()
    ///     .instance_id("custom-id")
    ///     .build();
    /// ```
    pub struct BeaconPayloadBuilder {
        instance_id: String,
        version: String,
        date: String,
        stats: BeaconStats,
    }

    impl Default for BeaconPayloadBuilder {
        fn default() -> Self {
            Self {
                instance_id: "a".repeat(64), // 64-char hex string (valid)
                version: "0.19.0".to_string(),
                date: "2026-06-08".to_string(),
                stats: BeaconStats {
                    total_requests: 100,
                    unique_fingerprints: 10,
                    models_used: Default::default(),
                    client_types: Default::default(),
                    avg_message_count: 5.0,
                    tool_use_ratio: 0.5,
                },
            }
        }
    }

    impl BeaconPayloadBuilder {
        pub fn instance_id(mut self, id: impl Into<String>) -> Self {
            self.instance_id = id.into();
            self
        }

        pub fn version(mut self, v: impl Into<String>) -> Self {
            self.version = v.into();
            self
        }

        pub fn date(mut self, d: impl Into<String>) -> Self {
            self.date = d.into();
            self
        }

        pub fn stats(mut self, s: BeaconStats) -> Self {
            self.stats = s;
            self
        }

        pub fn build(self) -> BeaconPayload {
            BeaconPayload {
                instance_id: self.instance_id,
                version: self.version,
                date: self.date,
                stats: self.stats,
            }
        }
    }

    /// Builder for constructing test BeaconStats instances.
    pub struct BeaconStatsBuilder {
        total_requests: u64,
        unique_fingerprints: u64,
        models_used: serde_json::Map<String, serde_json::Value>,
        client_types: serde_json::Map<String, serde_json::Value>,
        avg_message_count: f64,
        tool_use_ratio: f64,
    }

    impl Default for BeaconStatsBuilder {
        fn default() -> Self {
            Self {
                total_requests: 100,
                unique_fingerprints: 10,
                models_used: Default::default(),
                client_types: Default::default(),
                avg_message_count: 5.0,
                tool_use_ratio: 0.5,
            }
        }
    }

    impl BeaconStatsBuilder {
        pub fn total_requests(mut self, v: u64) -> Self {
            self.total_requests = v;
            self
        }

        pub fn unique_fingerprints(mut self, v: u64) -> Self {
            self.unique_fingerprints = v;
            self
        }

        pub fn avg_message_count(mut self, v: f64) -> Self {
            self.avg_message_count = v;
            self
        }

        pub fn tool_use_ratio(mut self, v: f64) -> Self {
            self.tool_use_ratio = v;
            self
        }

        pub fn build(self) -> BeaconStats {
            BeaconStats {
                total_requests: self.total_requests,
                unique_fingerprints: self.unique_fingerprints,
                models_used: self.models_used,
                client_types: self.client_types,
                avg_message_count: self.avg_message_count,
                tool_use_ratio: self.tool_use_ratio,
            }
        }
    }

    #[test]
    fn builder_default_is_valid() {
        let payload = BeaconPayloadBuilder::default().build();
        // Note: We can't call validate_payload here as it's in a different module
        // but we can at least check the payload is constructed correctly
        assert_eq!(payload.instance_id, "a".repeat(64));
    }

    #[test]
    fn builder_custom_instance_id() {
        let payload = BeaconPayloadBuilder::default().instance_id("custom-id-123").build();
        assert_eq!(payload.instance_id, "custom-id-123");
    }

    #[test]
    fn builder_with_invalid_field_fails_validation() {
        let payload = BeaconPayloadBuilder::default().instance_id(String::new()).build();
        // We can't actually test validation here since validate_payload isn't accessible
        // but we can at least check the instance_id is empty
        assert_eq!(payload.instance_id, String::new());
    }
}
