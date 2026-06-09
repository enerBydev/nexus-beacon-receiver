//! Domain layer — pure business logic with zero worker dependencies.
//!
//! This module contains all domain types, validation, security,
//! aggregation, rate limiting, port traits, and domain services.
//! No code in this module may depend on the `worker` crate.

pub mod aggregation;
pub mod cors;
pub mod ports;
pub mod rate_limit;
pub mod security;
pub mod services;
pub mod types;
pub mod validation;

#[allow(unused_imports)] // Used by D1Repository internally
pub use self::aggregation::{merge_json_objects, merge_versions};
#[allow(unused_imports)] // RateLimiter used via dyn trait in services
pub use self::rate_limit::RateLimiter;
pub use self::rate_limit::{BEACON_RATE_LIMITER, STATS_RATE_LIMITER};
pub use self::services::{AggregationService, BeaconService, StatsService};
pub use self::types::*;
#[allow(unused_imports)] // validate_payload used internally by services
pub use self::validation::validate_payload;
