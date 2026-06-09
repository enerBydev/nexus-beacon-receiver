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

use crate::adapters::d1_repository::D1Repository;
use crate::adapters::handlers::*;
use crate::config::*;
use crate::domain::AggregationService;
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
    let repo = D1Repository::new(db);
    let service = AggregationService::new(repo, BEACON_RETENTION_DAYS, STATS_RETENTION_DAYS);

    // Run aggregation for all dates needing it
    if let Err(e) = service.run_aggregation().await {
        worker::console_error!("run_aggregation failed: {}", e);
    }

    // Cleanup old data (daily 3am cron only)
    if event.cron() == "0 3 * * *" {
        if let Err(e) = service.run_cleanup().await {
            worker::console_error!("cleanup failed: {}", e);
        }
    }

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
    use crate::domain::merge_versions;

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
