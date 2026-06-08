//! WorkerCorsProvider — CorsProvider implementation.

use crate::domain::cors::parse_cors_origins;
use crate::domain::ports::CorsProvider;
use worker::RouteContext;

pub struct WorkerCorsProvider<'a> {
    ctx: &'a RouteContext<()>,
}

impl<'a> WorkerCorsProvider<'a> {
    pub fn new(ctx: &'a RouteContext<()>) -> Self {
        Self { ctx }
    }
}

impl CorsProvider for WorkerCorsProvider<'_> {
    fn origins(&self) -> Vec<String> {
        self.ctx.var("CORS_ORIGINS").map(|v| parse_cors_origins(&v.to_string())).unwrap_or_else(
            |_| {
                // Fail-closed: only allow production domains when config is missing
                parse_cors_origins("https://enerby.dev,https://www.enerby.dev")
            },
        )
    }

    fn methods(&self) -> Vec<String> {
        vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()]
    }

    fn allowed_headers(&self) -> Vec<String> {
        vec!["Content-Type".to_string(), "Authorization".to_string()]
    }

    fn max_age(&self) -> u32 {
        86400
    }
}
