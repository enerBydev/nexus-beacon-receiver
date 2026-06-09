//! Router configuration for the NEXUS Beacon Receiver worker.

use crate::adapters::handlers::*;
use worker::Router;

/// Configure the HTTP router with all route handlers.
pub fn configure_router() -> Router<'static, ()> {
    Router::new()
        .post_async("/v1/beacon", handle_beacon)
        .get_async("/v1/stats", handle_stats)
        .get_async("/v1/stats/summary", handle_summary)
        .get_async("/v1/stats/shield", handle_shield)
        .options("/*route", handle_options)
}
