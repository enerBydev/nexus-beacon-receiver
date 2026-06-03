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

use serde::{Deserialize, Serialize};
use worker::*;

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

/// Validate the `Authorization: Bearer <token>` header against the secret.
fn validate_auth(req: &Request, ctx: &RouteContext<()>) -> Result<()> {
    let expected = ctx.secret("BEACON_AUTH_TOKEN")?.to_string();
    let header = req.headers().get("Authorization")?.unwrap_or_default();
    let token = header.strip_prefix("Bearer ").unwrap_or(&header);
    if token == expected {
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

    // Recalculate daily_global_stats for this date from all beacons
    let recalc_stmt = worker::query!(
        &db,
        "INSERT OR REPLACE INTO daily_global_stats \
         (date, total_instances, total_requests, total_unique_users, \
          models_used, client_types, avg_message_count, tool_use_ratio, versions, updated_at) \
         SELECT \
             date, \
             COUNT(DISTINCT instance_id), \
             SUM(total_requests), \
             SUM(unique_fingerprints), \
             '{}', \
             '{}', \
             AVG(avg_message_count), \
             AVG(tool_use_ratio), \
             '{}', \
             DATETIME('now') \
         FROM beacons \
         WHERE date = ?1",
        &payload.date,
    )?;
    recalc_stmt.run().await?;

    Response::from_json(&serde_json::json!({"status": "ok"}))?.with_cors(&cors)
}

/// `GET /v1/stats` — return detailed stats for the last 30 days.
async fn handle_stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
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
        .options("/*", handle_options)
        .run(req, env)
        .await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {
        assert!(true);
    }
}
