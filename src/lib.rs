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

use crate::adapters::handlers::*;
use crate::config::*;
use crate::domain::*;
use worker::*;

// ---------------------------------------------------------------------------
// Scheduled aggregation (Cron Trigger) — Phase 16 will refactor this
// ---------------------------------------------------------------------------

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
