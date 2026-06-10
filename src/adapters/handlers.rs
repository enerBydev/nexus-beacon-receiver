//! Adapters — HTTP handlers and worker-specific implementations.

use crate::adapters::d1_repository::D1Repository;
use crate::adapters::worker_auth::WorkerAuthProvider;
use crate::adapters::worker_cors::WorkerCorsProvider;
use crate::config::*;
use crate::domain::ports::CorsProvider;
use crate::domain::*;
use worker::*;

/// Build the CORS configuration from WorkerCorsProvider.
pub(crate) fn cors_config(ctx: &RouteContext<()>) -> Cors {
    let provider = WorkerCorsProvider::new(ctx);
    Cors::new()
        .with_origins(provider.origins())
        .with_methods(
            provider
                .methods()
                .iter()
                .map(|m| match m.as_str() {
                    "GET" => Method::Get,
                    "POST" => Method::Post,
                    "OPTIONS" => Method::Options,
                    _ => Method::Get, // Default case, should never happen for the allowed methods
                })
                .collect::<Vec<_>>(),
        )
        .with_allowed_headers(provider.allowed_headers())
        .with_max_age(provider.max_age())
}

/// Create a JSON error response with CORS headers.
pub(crate) fn error_response(status: u16, msg: &'static str, cors: &Cors) -> Result<Response> {
    Response::from_json(&ErrorResponse { error: msg })?.with_status(status).with_cors(cors)
}

/// Map a BeaconResult to an HTTP response with CORS headers.
pub(crate) fn beacon_result_to_response(result: BeaconResult, cors: &Cors) -> Result<Response> {
    match result {
        BeaconResult::Success => {
            Response::from_json(&serde_json::json!({"status": "ok"}))?.with_cors(cors)
        }
        BeaconResult::RateLimited => error_response(429, "rate limit exceeded", cors),
        BeaconResult::Unauthorized => error_response(401, "unauthorized", cors),
        BeaconResult::InvalidContentType => error_response(415, "unsupported media type", cors),
        BeaconResult::PayloadTooLarge => error_response(413, "payload too large", cors),
        BeaconResult::InvalidPayload => error_response(400, "invalid payload", cors),
        BeaconResult::InternalError => error_response(500, "internal server error", cors),
    }
}

/// `POST /v1/beacon` — receive telemetry from a NEXUS-AI-Gateway instance.
pub async fn handle_beacon(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);

    // Content-Type validation — must be application/json
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    if !content_type.to_lowercase().starts_with("application/json") {
        return error_response(415, "unsupported media type", &cors);
    }

    // Body size limit check
    let content_length: usize =
        req.headers().get("Content-Length")?.unwrap_or_default().parse().unwrap_or(0);
    if content_length > MAX_BEACON_BODY_BYTES {
        return error_response(413, "payload too large", &cors);
    }

    // Parse payload
    let payload: BeaconPayload = match req.json().await {
        Ok(p) => p,
        Err(_) => {
            return error_response(400, "invalid payload", &cors);
        }
    };

    // Auth check
    let auth_provider = WorkerAuthProvider::new(&ctx);
    let auth_header = req.headers().get("Authorization")?.unwrap_or_default();

    let db = ctx.d1("DB")?;
    let repo = D1Repository::new(db);

    let service = BeaconService::new(
        repo,
        auth_provider,
        &BEACON_RATE_LIMITER,
        BEACON_MAX_PER_WINDOW,
        MAX_BEACON_BODY_BYTES,
    );
    let result =
        service.receive_beacon(&content_type, content_length, &auth_header, &payload).await;
    beacon_result_to_response(result, &cors)
}

/// `GET /v1/stats` — detailed statistics.
pub async fn handle_stats(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);

    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };

    let repo = D1Repository::new(db);
    let service = StatsService::new(repo, &STATS_RATE_LIMITER, STATS_MAX_PER_WINDOW);

    match service.get_stats().await {
        Ok(response) => Response::from_json(&response)?.with_cors(&cors),
        Err(BeaconResult::RateLimited) => error_response(429, "rate limit exceeded", &cors),
        Err(_) => error_response(500, "internal server error", &cors),
    }
}

/// `GET /v1/stats/summary` — lightweight aggregated numbers.
pub async fn handle_summary(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };
    let repo = D1Repository::new(db);
    let service = StatsService::new(repo, &STATS_RATE_LIMITER, STATS_MAX_PER_WINDOW);
    match service.get_summary().await {
        Ok(response) => Response::from_json(&response)?.with_cors(&cors),
        Err(BeaconResult::RateLimited) => error_response(429, "rate limit exceeded", &cors),
        Err(_) => error_response(500, "internal server error", &cors),
    }
}

/// `GET /v1/stats/shield` — Shields.io compatible badge data.
pub async fn handle_shield(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    let db = match ctx.d1("DB") {
        Ok(database) => database,
        Err(_) => return error_response(500, "internal server error", &cors),
    };
    let repo = D1Repository::new(db);
    let service = StatsService::new(repo, &STATS_RATE_LIMITER, STATS_MAX_PER_WINDOW);
    match service.get_shield().await {
        Ok(response) => Response::from_json(&response)?.with_cors(&cors),
        Err(BeaconResult::RateLimited) => error_response(429, "rate limit exceeded", &cors),
        Err(_) => error_response(500, "internal server error", &cors),
    }
}

/// `OPTIONS /*` — CORS preflight.
pub fn handle_options(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cors = cors_config(&ctx);
    Response::empty()?.with_status(204).with_cors(&cors)
}
