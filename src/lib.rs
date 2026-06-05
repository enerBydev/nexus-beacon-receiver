//! NEXUS Beacon Receiver - Cloudflare Worker for telemetry data collection
//!
//! This worker receives anonymous usage statistics from NEXUS AI Gateway instances
//! and aggregates them for global insights. It's designed to be lightweight and
//! privacy-preserving, collecting only non-identifiable metrics.
//!
//! Endpoints:
//! - POST /v1/beacon - Accepts telemetry data from gateways (auth required)
//! - GET /v1/stats - Returns detailed statistics (last 30 days)
//! - GET /v1/stats/summary - Returns summary statistics
//! - GET /v1/stats/shield - Returns Shields.io compatible badge data

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use worker::*;

// ---------------------------------------------------------------------------
// Rate limiting (in-memory, lock-free)
// ---------------------------------------------------------------------------

/// Beacon endpoint: 100 requests per window
static BEACON_COUNT: AtomicU32 = AtomicU32::new(0);
static BEACON_WINDOW_START: AtomicU64 = AtomicU64::new(0);
const BEACON_MAX_PER_WINDOW: u32 = 100;

/// Stats endpoints: 200 requests per window
static STATS_COUNT: AtomicU32 = AtomicU32::new(0);
static STATS_WINDOW_START: AtomicU64 = AtomicU64::new(0);
const STATS_MAX_PER_WINDOW: u32 = 200;

/// Window duration in approximate seconds.
/// CF Workers recycle every ~10min, so counters naturally reset.
const RATE_WINDOW_SECS: u64 = 60;

/// Monotonic counter for approximate timekeeping (CF Workers WASM lacks SystemTime).
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Get approximate current time in seconds.
/// Uses a global request counter divided by an estimated requests-per-second rate.
/// This is imprecise but sufficient for defense-in-depth rate limiting.
/// CF Rate Limiting Rules (platform-level) provide precise time-based limiting.
fn now_approx_secs() -> u64 {
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    // Rough approximation: ~100 requests/sec for a telemetry worker
    count / 100
}

/// Check if a request is allowed under the rate limit.
/// Uses atomic counters for lock-free concurrent access.
/// Returns false if the rate limit has been exceeded.
fn check_rate_limit(count: &AtomicU32, window_start: &AtomicU64, max_per_window: u32) -> bool {
    let now = now_approx_secs();
    let last_reset = window_start.load(Ordering::Relaxed);

    // Reset window if expired
    if now > last_reset && now - last_reset > RATE_WINDOW_SECS {
        // CAS to prevent double-reset race between concurrent requests
        if window_start
            .compare_exchange(last_reset, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            count.store(0, Ordering::Relaxed);
        }
    }

    let current = count.fetch_add(1, Ordering::Relaxed);
    current < max_per_window
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Incoming beacon payload from a NEXUS-AI-Gateway instance.
#[derive(Deserialize)]
struct BeaconPayload {
    instance_id: String,
    version: String,
    date: String,
    stats: BeaconStats,
}

/// Per-instance daily statistics inside the beacon payload.
#[derive(Deserialize)]
struct BeaconStats {
    total_requests: u64,
    unique_fingerprints: u64,
    #[serde(default)]
    models_used: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    client_types: serde_json::Map<String, serde_json::Value>,
    avg_message_count: f64,
    tool_use_ratio: f64,
}

/// Row from the `beacons` table used for JSON merge aggregation.
#[derive(Deserialize)]
struct BeaconRow {
    models_used: String,
    client_types: String,
    version: String,
}

/// Merge multiple JSON objects from beacons into a single aggregated object.
/// Each numeric value is summed across all objects (supports both integers and floats).
fn merge_json_objects(json_strings: &[String]) -> String {
    let mut merged: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for js in json_strings {
        if let Ok(obj) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(js) {
            for (key, value) in obj {
                let incoming = value.as_f64().unwrap_or(0.0);
                let existing = merged.entry(key).or_insert_with(|| serde_json::Value::from(0));
                // Preserve integer representation when possible for cleaner output
                if let Some(n) = existing.as_f64() {
                    let sum = n + incoming;
                    if sum.fract() == 0.0 {
                        *existing = serde_json::Value::from(sum as i64);
                    } else {
                        *existing = serde_json::Value::from(sum);
                    }
                }
            }
        }
    }
    serde_json::to_string(&merged).unwrap_or_else(|_| "{}".to_string())
}

/// Merge version strings from beacons into a `{version: instance_count}` JSON.
fn merge_versions(versions: &[String]) -> String {
    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for v in versions {
        let v = v.trim().to_string();
        if !v.is_empty() {
            *counts.entry(v).or_insert(0) += 1;
        }
    }
    serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string())
}

/// Result of numeric aggregation query for daily_global_stats.
#[derive(Deserialize)]
struct AggregationResult {
    total_instances: i64,
    total_requests: i64,
    total_unique_users: i64,
    avg_message_count: f64,
    tool_use_ratio: f64,
}

/// Row from the `daily_global_stats` D1 table.
#[derive(Serialize, Deserialize)]
struct DailyGlobalStats {
    date: String,
    total_instances: i64,
    total_requests: i64,
    total_unique_users: i64,
    models_used: String,
    client_types: String,
    avg_message_count: f64,
    tool_use_ratio: f64,
    versions: String,
}

/// Response for `GET /v1/stats`.
#[derive(Serialize)]
struct StatsResponse {
    stats: Vec<DailyGlobalStats>,
}

/// Response for `GET /v1/stats/summary`.
#[derive(Serialize, Deserialize)]
struct SummaryResponse {
    total_instances: i64,
    total_requests: i64,
    total_unique_users: i64,
    days_active: i64,
}

/// Generic error response body.
#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

/// Response for `GET /v1/stats/shield` - Shields.io endpoint badge format.
#[derive(Serialize)]
struct ShieldResponse {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: &'static str,
    message: String,
    color: &'static str,
    #[serde(rename = "namedLogo")]
    named_logo: &'static str,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the CORS configuration from the `CORS_ORIGINS` env var.
fn cors_config(ctx: &RouteContext<()>) -> Cors {
    let origins: Vec<String> = ctx
        .var("CORS_ORIGINS")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "*".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    Cors::new()
        .with_origins(origins)
        .with_methods(vec![Method::Get, Method::Post, Method::Options])
        .with_allowed_headers(vec!["Content-Type", "Authorization"])
        .with_max_age(86400)
}

/// Create a JSON error response with CORS headers.
fn error_response(status: u16, msg: &'static str, cors: &Cors) -> Result<Response> {
    Response::from_json(&ErrorResponse { error: msg })?.with_status(status).with_cors(cors)
}

/// Constant-time string comparison resistant to timing side-channel attacks.
/// XORs all bytes and ORs length difference so comparison time is independent
/// of where strings differ. Does NOT use ring/subtle (won't compile to wasm32).
fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result: u8 = 0;
    for i in 0..a_bytes.len().min(b_bytes.len()) {
        result |= a_bytes[i] ^ b_bytes[i];
    }
    result |= (a_bytes.len() != b_bytes.len()) as u8;
    result == 0
}

/// Case-insensitive "Bearer " prefix extraction from Authorization header.
/// Per RFC 7235, the auth-scheme token is case-insensitive.
fn extract_bearer_token(header: &str) -> &str {
    if header.len() >= 7 {
        let prefix = &header[..7];
        if prefix.eq_ignore_ascii_case("Bearer ") {
            return &header[7..];
        }
    }
    header
}

/// Overwrite a String's heap memory with zeroes to prevent credential leakage
/// after comparison. The String is then cleared to length 0.
fn zeroize_string(s: &mut String) {
    let bytes = unsafe { s.as_bytes_mut() };
    for byte in bytes.iter_mut() {
        *byte = 0;
    }
    s.clear();
}

/// Validate the `Authorization: Bearer <token>` header against the secret.
fn validate_auth(req: &Request, ctx: &RouteContext<()>) -> Result<()> {
    let mut expected = ctx.secret("BEACON_AUTH_TOKEN")?.to_string();
    let header = req.headers().get("Authorization")?.unwrap_or_default();
    let token = extract_bearer_token(&header);

    let is_valid = constant_time_eq(token, &expected);
    zeroize_string(&mut expected);

    if is_valid {
        Ok(())
    } else {
        Err(Error::RustError("unauthorized".into()))
    }
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// `POST /v1/beacon` — receive telemetry from a NEXUS-AI-Gateway instance.
async fn handle_beacon(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);

    // Rate limit check (before auth to prevent auth-targeted flooding)
    if !check_rate_limit(&BEACON_COUNT, &BEACON_WINDOW_START, BEACON_MAX_PER_WINDOW) {
        return error_response(429, "rate limit exceeded", &cors);
    }

    // Auth check
    if validate_auth(&req, &ctx).is_err() {
        return error_response(401, "unauthorized", &cors);
    }

    // Parse payload
    let payload: BeaconPayload = match req.json().await {
        Ok(p) => p,
        Err(_) => return error_response(400, "invalid payload", &cors),
    };

    // Basic validation
    if payload.instance_id.is_empty() || payload.date.is_empty() || payload.version.is_empty() {
        return error_response(400, "missing required field", &cors);
    }

    let db = ctx.d1("DB")?;

    let models_json =
        serde_json::to_string(&payload.stats.models_used).unwrap_or_else(|_| "{}".to_string());
    let client_types_json =
        serde_json::to_string(&payload.stats.client_types).unwrap_or_else(|_| "{}".to_string());

    // Upsert beacon row
    let upsert_stmt = worker::query!(
        &db,
        "INSERT OR REPLACE INTO beacons \
         (instance_id, version, date, total_requests, unique_fingerprints, \
          models_used, client_types, avg_message_count, tool_use_ratio) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        &payload.instance_id,
        &payload.version,
        &payload.date,
        &payload.stats.total_requests,
        &payload.stats.unique_fingerprints,
        &models_json,
        &client_types_json,
        &payload.stats.avg_message_count,
        &payload.stats.tool_use_ratio,
    )?;
    upsert_stmt.run().await?;

    // Recalculate daily_global_stats for this date from all beacons.
    // JSON merge is done in Rust because D1/SQLite lacks native JSON merge.
    let merge_rows = worker::query!(
        &db,
        "SELECT models_used, client_types, version FROM beacons WHERE date = ?1",
        &payload.date,
    )?;
    let merge_result = merge_rows.all().await?;
    let rows: Vec<BeaconRow> = merge_result.results()?;

    let models_json =
        merge_json_objects(&rows.iter().map(|r| r.models_used.clone()).collect::<Vec<_>>());
    let ct_json =
        merge_json_objects(&rows.iter().map(|r| r.client_types.clone()).collect::<Vec<_>>());
    let ver_json = merge_versions(&rows.iter().map(|r| r.version.clone()).collect::<Vec<_>>());

    // Get aggregate counts via SQL
    let count_stmt = worker::query!(
        &db,
        "SELECT COUNT(DISTINCT instance_id) as total_instances, \
         SUM(total_requests) as total_requests, \
         SUM(unique_fingerprints) as total_unique_users, \
         AVG(avg_message_count) as avg_message_count, \
         AVG(tool_use_ratio) as tool_use_ratio \
         FROM beacons WHERE date = ?1",
        &payload.date,
    )?;
    let agg: AggregationResult = count_stmt.first(None).await?.unwrap_or(AggregationResult {
        total_instances: 0,
        total_requests: 0,
        total_unique_users: 0,
        avg_message_count: 0.0,
        tool_use_ratio: 0.0,
    });

    let recalc_stmt = worker::query!(
        &db,
        "INSERT OR REPLACE INTO daily_global_stats \
         (date, total_instances, total_requests, total_unique_users, \
          models_used, client_types, avg_message_count, tool_use_ratio, versions, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, DATETIME('now'))",
        &payload.date,
        agg.total_instances,
        agg.total_requests,
        agg.total_unique_users,
        &models_json,
        &ct_json,
        agg.avg_message_count,
        agg.tool_use_ratio,
        &ver_json,
    )?;
    recalc_stmt.run().await?;

    Response::from_json(&serde_json::json!({"status": "ok"}))?.with_cors(&cors)
}

/// `GET /v1/stats` — return detailed stats for the last 30 days.
async fn handle_stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    if !check_rate_limit(&STATS_COUNT, &STATS_WINDOW_START, STATS_MAX_PER_WINDOW) {
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = ctx.d1("DB")?;

    let stmt = worker::query!(
        &db,
        "SELECT date, total_instances, total_requests, total_unique_users, \
         models_used, client_types, avg_message_count, tool_use_ratio, versions \
         FROM daily_global_stats \
         ORDER BY date DESC \
         LIMIT 30",
    )?;
    let result = stmt.all().await?;
    let stats: Vec<DailyGlobalStats> = result.results()?;

    Response::from_json(&StatsResponse { stats })?.with_cors(&cors)
}

/// `GET /v1/stats/summary` — lightweight aggregated numbers.
async fn handle_summary(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    if !check_rate_limit(&STATS_COUNT, &STATS_WINDOW_START, STATS_MAX_PER_WINDOW) {
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = ctx.d1("DB")?;

    let stmt = worker::query!(
        &db,
        "SELECT \
             COALESCE(SUM(total_instances), 0) as total_instances, \
             COALESCE(SUM(total_requests), 0) as total_requests, \
             COALESCE(SUM(total_unique_users), 0) as total_unique_users, \
             COUNT(*) as days_active \
         FROM daily_global_stats",
    )?;
    let summary: SummaryResponse = stmt.first(None).await?.unwrap_or(SummaryResponse {
        total_instances: 0,
        total_requests: 0,
        total_unique_users: 0,
        days_active: 0,
    });

    Response::from_json(&summary)?.with_cors(&cors)
}

/// `GET /v1/stats/shield` — Shields.io compatible badge data.
async fn handle_shield(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    if !check_rate_limit(&STATS_COUNT, &STATS_WINDOW_START, STATS_MAX_PER_WINDOW) {
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = ctx.d1("DB")?;

    let stmt = worker::query!(
        &db,
        "SELECT COALESCE(SUM(total_instances), 0) as total_instances FROM daily_global_stats",
    )?;
    let row: serde_json::Value = stmt.first(None).await?.unwrap_or_default();
    let total = row.get("total_instances").and_then(|v| v.as_i64()).unwrap_or(0);

    let shield = ShieldResponse {
        schema_version: 1,
        label: "NEXUS",
        message: format!("{} active", total),
        color: "blue",
        named_logo: "cloudflare",
    };

    Response::from_json(&shield)?.with_cors(&cors)
}

/// `OPTIONS /*` — CORS preflight.
fn handle_options(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    Response::empty()?.with_status(204).with_cors(&cors)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .post_async("/v1/beacon", handle_beacon)
        .get_async("/v1/stats", handle_stats)
        .get_async("/v1/stats/summary", handle_summary)
        .get_async("/v1/stats/shield", handle_shield)
        .options("/*route", handle_options)
        .run(req, env)
        .await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_compiles() {
        assert!(true);
    }

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
    fn merge_versions_empty() {
        assert_eq!(merge_versions(&[]), "{}");
    }

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
    fn zeroize_string_clears_content() {
        let mut s = String::from("secret_token_value");
        zeroize_string(&mut s);
        assert_eq!(s.len(), 0);
        assert_eq!(s, "");
    }

    #[test]
    fn rate_limit_allows_under_limit() {
        static TEST_COUNT: AtomicU32 = AtomicU32::new(0);
        static TEST_WINDOW: AtomicU64 = AtomicU64::new(0);
        TEST_COUNT.store(0, Ordering::Relaxed);
        TEST_WINDOW.store(0, Ordering::Relaxed);
        REQUEST_COUNTER.store(0, Ordering::Relaxed);

        for _ in 0..5 {
            assert!(check_rate_limit(&TEST_COUNT, &TEST_WINDOW, 10));
        }
    }

    #[test]
    fn rate_limit_blocks_over_limit() {
        static TEST_COUNT: AtomicU32 = AtomicU32::new(0);
        static TEST_WINDOW: AtomicU64 = AtomicU64::new(0);
        TEST_COUNT.store(0, Ordering::Relaxed);
        TEST_WINDOW.store(0, Ordering::Relaxed);
        REQUEST_COUNTER.store(10000, Ordering::Relaxed); // Avoid window reset during test

        for _ in 0..10 {
            check_rate_limit(&TEST_COUNT, &TEST_WINDOW, 10);
        }
        // 11th request should be blocked
        assert!(!check_rate_limit(&TEST_COUNT, &TEST_WINDOW, 10));
    }
}
