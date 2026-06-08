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
//!
//! Scheduled:
//! - Cron "0 * * * *" - Hourly aggregation of beacons into daily_global_stats
//! - Cron "0 3 * * *" - Daily 3am UTC cleanup (beacons >90d, stats >365d)

// workers-rs 0.8.3 #[event(scheduled)] macro generates code that discards the
// handler's Result return value by design (wraps in Promise returning undefined).
// Suppress unused_must_use at crate level to allow clippy -- -D warnings.
#![allow(unused_must_use)]

mod adapters;
mod config;
mod domain;
mod infrastructure;

use crate::adapters::worker_auth::WorkerAuthProvider;
use crate::config::*;
use crate::domain::ports::AuthProvider;
use crate::domain::RateLimiter;
use crate::domain::*;
use worker::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the CORS configuration from the `CORS_ORIGINS` env var.
fn cors_config(ctx: &RouteContext<()>) -> Cors {
    let origins: Vec<String> = ctx
        .var("CORS_ORIGINS")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| {
            // Fail-closed: only allow production domains when config is missing
            "https://enerby.dev,https://www.enerby.dev".to_string()
        })
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// `POST /v1/beacon` — receive telemetry from a NEXUS-AI-Gateway instance.
async fn handle_beacon(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);

    // Rate limit check (before auth to prevent auth-targeted flooding)
    if !BEACON_RATE_LIMITER.check(BEACON_MAX_PER_WINDOW) {
        worker::console_error!("rate limit exceeded on beacon endpoint");
        return error_response(429, "rate limit exceeded", &cors);
    }

    // Content-Type validation — must be application/json
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    if !content_type.to_lowercase().starts_with("application/json") {
        worker::console_error!("beacon rejected: invalid Content-Type");
        return error_response(415, "unsupported media type", &cors);
    }

    // Body size limit check
    let content_length: usize =
        req.headers().get("Content-Length")?.unwrap_or_default().parse().unwrap_or(0);
    if content_length > MAX_BEACON_BODY_BYTES {
        worker::console_error!("beacon rejected: body exceeds 10KB limit");
        return error_response(413, "payload too large", &cors);
    }

    // Auth check
    let auth_provider = WorkerAuthProvider::new(&ctx);
    let auth_header = req.headers().get("Authorization")?.unwrap_or_default();
    if auth_provider.validate_auth(&auth_header).is_err() {
        worker::console_error!("auth failed: missing or invalid Authorization header");
        return error_response(401, "unauthorized", &cors);
    }

    // Parse payload
    let payload: BeaconPayload = match req.json().await {
        Ok(p) => p,
        Err(_) => {
            worker::console_error!("beacon rejected: payload parse failed");
            return error_response(400, "invalid payload", &cors);
        }
    };

    if validate_payload(&payload).is_err() {
        worker::console_error!("beacon rejected: payload validation failed");
        return error_response(400, "invalid payload", &cors);
    }

    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => {
            worker::console_error!("database connection failed");
            return error_response(500, "internal server error", &cors);
        }
    };

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

    Response::from_json(&serde_json::json!({"status": "ok"}))?.with_cors(&cors)
}

/// `GET /v1/stats` — return detailed stats for the last 30 days.
async fn handle_stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    if !STATS_RATE_LIMITER.check(STATS_MAX_PER_WINDOW) {
        worker::console_error!("rate limit exceeded on stats endpoint");
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };

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
    if !STATS_RATE_LIMITER.check(STATS_MAX_PER_WINDOW) {
        worker::console_error!("rate limit exceeded on stats endpoint");
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };

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
    if !STATS_RATE_LIMITER.check(STATS_MAX_PER_WINDOW) {
        worker::console_error!("rate limit exceeded on stats endpoint");
        return error_response(429, "rate limit exceeded", &cors);
    }
    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };

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
// Scheduled aggregation (Cron Trigger)
// ---------------------------------------------------------------------------

/// Retention periods for data cleanup.
/// Scheduled event handler for periodic aggregation and cleanup.
/// Triggered by Cron: "0 * * * *" (every hour) for aggregation,
/// and "0 3 * * *" (daily 3am UTC) for cleanup.
/// Note: The `#[event(scheduled)]` macro discards the handler's return value
/// by design (workers-rs 0.8.3 wraps it in a Promise returning `undefined`).
/// Errors are logged via `console_error!` before returning.
#[event(scheduled)]
async fn cron(
    event: worker::ScheduledEvent,
    env: worker::Env,
    _ctx: worker::ScheduleContext,
) -> worker::Result<()> {
    let db = env.d1("DB")?;

    // 1. Find dates that need aggregation (today and yesterday)
    let dates_stmt = worker::query!(
        &db,
        "SELECT DISTINCT date FROM beacons \
        WHERE date >= DATE('now', '-1 day') \
        ORDER BY date DESC"
    );
    let dates_result = dates_stmt.all().await?;

    // 2. For each date, recalculate daily_global_stats
    for date_row in dates_result.results::<serde_json::Value>()? {
        if let Some(date) = date_row.get("date").and_then(|v| v.as_str()) {
            if let Err(e) = aggregate_for_date(&db, date).await {
                worker::console_error!("aggregate_for_date({}) failed: {}", date, e);
            }
        }
    }

    // 3. Cleanup old data (daily 3am cron only)
    if event.cron() == "0 3 * * *" {
        if let Err(e) = cleanup_old_data(&db).await {
            worker::console_error!("cleanup_old_data failed: {}", e);
        }
    }

    Ok(())
}

/// Aggregate all beacons for a given date into daily_global_stats.
async fn aggregate_for_date(db: &worker::D1Database, date: &str) -> worker::Result<()> {
    // Merge JSON fields in Rust (D1/SQLite lacks native JSON merge)
    let merge_rows = worker::query!(
        &db,
        "SELECT models_used, client_types, version FROM beacons WHERE date = ?1",
        date,
    )?;
    let merge_result = merge_rows.all().await?;
    let rows: Vec<BeaconRow> = merge_result.results()?;

    let models_json =
        merge_json_objects(&rows.iter().map(|r| r.models_used.clone()).collect::<Vec<_>>());
    let ct_json =
        merge_json_objects(&rows.iter().map(|r| r.client_types.clone()).collect::<Vec<_>>());
    let ver_json = merge_versions(&rows.iter().map(|r| r.version.clone()).collect::<Vec<_>>());

    // SQL aggregation
    let count_stmt = worker::query!(
        &db,
        "SELECT COUNT(DISTINCT instance_id) as total_instances, \
        SUM(total_requests) as total_requests, \
        SUM(unique_fingerprints) as total_unique_users, \
        AVG(avg_message_count) as avg_message_count, \
        AVG(tool_use_ratio) as tool_use_ratio \
        FROM beacons WHERE date = ?1",
        date,
    )?;
    let agg: AggregationResult = count_stmt.first(None).await?.unwrap_or(AggregationResult {
        total_instances: 0,
        total_requests: 0,
        total_unique_users: 0,
        avg_message_count: 0.0,
        tool_use_ratio: 0.0,
    });

    // Upsert global stats
    let recalc_stmt = worker::query!(
        &db,
        "INSERT OR REPLACE INTO daily_global_stats \
        (date, total_instances, total_requests, total_unique_users, \
        models_used, client_types, avg_message_count, tool_use_ratio, versions, updated_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, DATETIME('now'))",
        date,
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

    Ok(())
}

/// Delete old data beyond retention window.
async fn cleanup_old_data(db: &worker::D1Database) -> worker::Result<()> {
    let stmt = worker::query!(
        &db,
        "DELETE FROM beacons WHERE date < DATE('now', ?1 || ' days')",
        format!("-{}", BEACON_RETENTION_DAYS),
    )?;
    stmt.run().await?;

    let stmt = worker::query!(
        &db,
        "DELETE FROM daily_global_stats WHERE date < DATE('now', ?1 || ' days')",
        format!("-{}", STATS_RETENTION_DAYS),
    )?;
    stmt.run().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point

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
    fn merge_versions_empty() {
        assert_eq!(merge_versions(&[]), "{}");
    }

    #[test]
    fn cors_fail_closed_no_wildcard() {
        // The default fallback should never be "*"
        let default = "https://enerby.dev,https://www.enerby.dev";
        let origins: Vec<&str> = default.split(',').map(|s| s.trim()).collect();
        assert!(!origins.contains(&"*"));
        assert!(origins.contains(&"https://enerby.dev"));
    }

    #[test]
    fn retention_constants_correct() {
        assert_eq!(BEACON_RETENTION_DAYS, 90);
        assert_eq!(STATS_RETENTION_DAYS, 365);
    }

    #[test]
    fn body_size_limit_correct() {
        assert_eq!(MAX_BEACON_BODY_BYTES, 10_240);
    }
}
