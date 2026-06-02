//! NEXUS Beacon Receiver - Cloudflare Worker for telemetry data collection
//!
//! This worker receives anonymous usage statistics from NEXUS AI Gateway instances
//! and aggregates them for global insights. It's designed to be lightweight and
//! privacy-preserving, collecting only non-identifiable metrics.
//!
//! Endpoints:
//! - POST /v1/beacon - Accepts telemetry data from gateways
//! - GET /v1/stats - Returns detailed statistics
//! - GET /v1/stats/summary - Returns summary statistics

use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .post("/v1/beacon", |_, _| Response::error("not implemented", 501))
        .get("/v1/stats", |_, _| Response::error("not implemented", 501))
        .get("/v1/stats/summary", |_, _| Response::error("not implemented", 501))
        .run(req, env)
        .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {
        // This test ensures the worker compiles correctly
        assert!(true);
    }
}
