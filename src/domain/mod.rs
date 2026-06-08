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

pub use self::aggregation::merge_json_objects;
pub use self::aggregation::merge_versions;
pub use self::rate_limit::{RateLimiter, BEACON_RATE_LIMITER, STATS_RATE_LIMITER};
pub use self::types::*;
pub use self::validation::validate_payload;
