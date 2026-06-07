//! Domain layer — pure business logic with zero worker dependencies.
//!
//! This module contains all domain types, validation, security,
//! aggregation, rate limiting, port traits, and domain services.
//! No code in this module may depend on the `worker` crate.

pub mod aggregation;
pub mod ports;
pub mod rate_limit;
pub mod security;
pub mod services;
pub mod types;
pub mod validation;
