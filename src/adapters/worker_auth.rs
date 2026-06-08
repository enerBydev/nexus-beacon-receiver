//! WorkerAuthProvider — AuthProvider implementation via RouteContext.

use crate::domain::ports::AuthProvider;
use crate::domain::security::verify_bearer_token;
use crate::domain::types::AuthError;
use worker::RouteContext;

pub struct WorkerAuthProvider<'a> {
    ctx: &'a RouteContext<()>,
}

impl<'a> WorkerAuthProvider<'a> {
    pub fn new(ctx: &'a RouteContext<()>) -> Self {
        Self { ctx }
    }
}

impl AuthProvider for WorkerAuthProvider<'_> {
    fn validate_auth(&self, auth_header: &str) -> Result<(), AuthError> {
        let secret = self
            .ctx
            .secret("BEACON_AUTH_TOKEN")
            .map_err(|_| AuthError::InvalidCredentials)?
            .to_string();
        verify_bearer_token(auth_header, &secret)
    }
}
