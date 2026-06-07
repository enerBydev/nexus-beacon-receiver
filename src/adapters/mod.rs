//! Adapter layer — I/O implementations for worker infrastructure.
//!
//! This module contains handlers, D1 repository, auth provider,
//! and CORS provider that implement the domain port traits.

pub mod d1_repository;
pub mod handlers;
pub mod worker_auth;
pub mod worker_cors;
