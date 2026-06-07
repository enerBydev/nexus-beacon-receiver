//! Configuration constants and SQL strings.

/// Maximum field lengths to prevent D1 storage abuse.
pub(crate) const MAX_INSTANCE_ID_LEN: usize = 64; // HMAC-SHA256 hex = 64 chars
pub(crate) const MAX_VERSION_LEN: usize = 24; // "0.99.99-dev+metadata"
pub(crate) const MAX_DATE_LEN: usize = 10; // "YYYY-MM-DD"
pub(crate) const MAX_MAP_ENTRIES: usize = 50; // Max entries in models_used/client_types
pub(crate) const MAX_TOTAL_REQUESTS: u64 = 10_000_000;
pub(crate) const MAX_UNIQUE_FINGERPRINTS: u64 = 100_000;

/// Rate limiting (in-memory, lock-free)
pub(crate) const BEACON_MAX_PER_WINDOW: u32 = 100;
pub(crate) const STATS_MAX_PER_WINDOW: u32 = 200;
pub(crate) const RATE_WINDOW_SECS: u64 = 60;
pub(crate) const MAX_BEACON_BODY_BYTES: usize = 10_240;

/// Retention periods for data cleanup.
pub(crate) const BEACON_RETENTION_DAYS: i64 = 90;
pub(crate) const STATS_RETENTION_DAYS: i64 = 365;

/// Upsert a beacon row into the `beacons` table.
#[allow(dead_code)]
pub(crate) const SQL_UPSERT_BEACON: &str = "\
    INSERT OR REPLACE INTO beacons \
    (instance_id, version, date, total_requests, unique_fingerprints, \
    models_used, client_types, avg_message_count, tool_use_ratio) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

/// Get daily stats for the last 30 days.
#[allow(dead_code)]
pub(crate) const SQL_GET_DAILY_STATS: &str = "\
    SELECT date, total_instances, total_requests, total_unique_users, \
    models_used, client_types, avg_message_count, tool_use_ratio, versions \
    FROM daily_global_stats \
    ORDER BY date DESC \
    LIMIT 30";

/// Get summary statistics across all days.
#[allow(dead_code)]
pub(crate) const SQL_GET_SUMMARY: &str = "\
    SELECT \
    COALESCE(SUM(total_instances), 0) as total_instances, \
    COALESCE(SUM(total_requests), 0) as total_requests, \
    COALESCE(SUM(total_unique_users), 0) as total_unique_users, \
    COUNT(*) as days_active \
    FROM daily_global_stats";

/// Get total instances for Shields.io badge.
#[allow(dead_code)]
pub(crate) const SQL_GET_TOTAL_INSTANCES: &str = "\
    SELECT COALESCE(SUM(total_instances), 0) as total_instances FROM daily_global_stats";

#[allow(dead_code)]
pub(crate) const SQL_GET_DATES_FOR_AGGREGATION: &str = "\
    SELECT DISTINCT date FROM beacons \
    WHERE date >= DATE('now', '-1 day') \
    ORDER BY date DESC";

/// Get beacons for a date to merge JSON fields.
#[allow(dead_code)]
pub(crate) const SQL_GET_BEACONS_FOR_DATE: &str = "\
    SELECT models_used, client_types, version FROM beacons WHERE date = ?1";

/// Aggregate numeric stats for a date.
#[allow(dead_code)]
pub(crate) const SQL_AGGREGATE_DATE: &str = "\
    SELECT COUNT(DISTINCT instance_id) as total_instances, \
    SUM(total_requests) as total_requests, \
    SUM(unique_fingerprints) as total_unique_users, \
    AVG(avg_message_count) as avg_message_count, \
    AVG(tool_use_ratio) as tool_use_ratio \
    FROM beacons WHERE date = ?1";

/// Upsert global stats for a date.
#[allow(dead_code)]
pub(crate) const SQL_UPSERT_GLOBAL_STATS: &str = "\
    INSERT OR REPLACE INTO daily_global_stats \
    (date, total_instances, total_requests, total_unique_users, \
    models_used, client_types, avg_message_count, tool_use_ratio, versions, updated_at) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, DATETIME('now'))";

/// Delete old beacons beyond retention window.
#[allow(dead_code)]
pub(crate) const SQL_CLEANUP_BEACONS: &str = "\
    DELETE FROM beacons WHERE date < DATE('now', ?1 || ' days')";

/// Delete old daily stats beyond retention window.
#[allow(dead_code)]
pub(crate) const SQL_CLEANUP_STATS: &str = "\
    DELETE FROM daily_global_stats WHERE date < DATE('now', ?1 || ' days')";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_max_instance_id_len() {
        assert_eq!(MAX_INSTANCE_ID_LEN, 64);
    }

    #[test]
    fn config_max_version_len() {
        assert_eq!(MAX_VERSION_LEN, 24);
    }

    #[test]
    fn config_max_date_len() {
        assert_eq!(MAX_DATE_LEN, 10);
    }

    #[test]
    fn config_max_map_entries() {
        assert_eq!(MAX_MAP_ENTRIES, 50);
    }

    #[test]
    fn config_max_total_requests() {
        assert_eq!(MAX_TOTAL_REQUESTS, 10_000_000);
    }

    #[test]
    fn config_max_unique_fingerprints() {
        assert_eq!(MAX_UNIQUE_FINGERPRINTS, 100_000);
    }

    #[test]
    fn config_beacon_max_per_window() {
        assert_eq!(BEACON_MAX_PER_WINDOW, 100);
    }

    #[test]
    fn config_stats_max_per_window() {
        assert_eq!(STATS_MAX_PER_WINDOW, 200);
    }

    #[test]
    fn config_rate_window_secs() {
        assert_eq!(RATE_WINDOW_SECS, 60);
    }

    #[test]
    fn config_max_beacon_body_bytes() {
        assert_eq!(MAX_BEACON_BODY_BYTES, 10_240);
    }

    #[test]
    fn config_beacon_retention_days() {
        assert_eq!(BEACON_RETENTION_DAYS, 90);
    }

    #[test]
    fn config_stats_retention_days() {
        assert_eq!(STATS_RETENTION_DAYS, 365);
    }

    #[test]
    fn config_sql_strings_are_non_empty() {
        assert!(!SQL_UPSERT_BEACON.is_empty());
        assert!(!SQL_GET_DAILY_STATS.is_empty());
        assert!(!SQL_GET_SUMMARY.is_empty());
        assert!(!SQL_GET_TOTAL_INSTANCES.is_empty());
        assert!(!SQL_GET_DATES_FOR_AGGREGATION.is_empty());
        assert!(!SQL_GET_BEACONS_FOR_DATE.is_empty());
        assert!(!SQL_AGGREGATE_DATE.is_empty());
        assert!(!SQL_UPSERT_GLOBAL_STATS.is_empty());
        assert!(!SQL_CLEANUP_BEACONS.is_empty());
        assert!(!SQL_CLEANUP_STATS.is_empty());
    }
}
