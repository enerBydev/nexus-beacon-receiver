//! Domain services (use case orchestration).
//!
//! Each service encapsulates a use case by orchestrating domain logic
//! through port traits. Services have zero worker dependencies — they
//! are fully testable with mockall.

use crate::domain::ports::{AuthProvider, BeaconRepository};
use crate::domain::rate_limit::RateLimiter;
use crate::domain::types::{
    BeaconPayload, BeaconResult, RepositoryError, ShieldResponse, StatsResponse, SummaryResponse,
};
use crate::domain::validation::validate_payload;

// ---------------------------------------------------------------------------
// BeaconService
// ---------------------------------------------------------------------------

/// Service for receiving beacon telemetry from NEXUS-AI-Gateway instances.
///
/// # Example
/// ```rust,ignore
/// // BeaconService orchestrates: rate limit → content-type → body-size → auth → validate → repo upsert
/// // Requires a BeaconRepository and AuthProvider implementation.
/// // See domain::services tests for mockall-based usage examples.
/// ```
pub struct BeaconService<'a, R: BeaconRepository, A: AuthProvider> {
    repo: R,
    auth: A,
    rate_limiter: &'a dyn RateLimiter,
    beacon_max_per_window: u32,
    max_body_bytes: usize,
}

impl<'a, R: BeaconRepository, A: AuthProvider> BeaconService<'a, R, A> {
    /// Create a new BeaconService with the given dependencies.
    pub fn new(
        repo: R,
        auth: A,
        rate_limiter: &'a dyn RateLimiter,
        beacon_max_per_window: u32,
        max_body_bytes: usize,
    ) -> Self {
        Self { repo, auth, rate_limiter, beacon_max_per_window, max_body_bytes }
    }

    /// Receive a beacon payload and process it through the full validation pipeline.
    ///
    /// # Example
    /// ```rust,ignore
    /// // BeaconService orchestrates: rate limit → content-type → body-size → auth → validate → repo upsert
    /// // The caller (handler) is responsible for parsing the JSON body into a BeaconPayload before calling this method.
    /// ```
    pub async fn receive_beacon(
        &self,
        content_type: &str,
        content_length: usize,
        auth_header: &str,
        payload: &BeaconPayload,
    ) -> BeaconResult {
        // 1. Rate limit
        if !self.rate_limiter.check(self.beacon_max_per_window) {
            return BeaconResult::RateLimited;
        }

        // 2. Content-Type validation
        if !content_type.to_lowercase().starts_with("application/json") {
            return BeaconResult::InvalidContentType;
        }

        // 3. Body size check
        if content_length > self.max_body_bytes {
            return BeaconResult::PayloadTooLarge;
        }

        // 4. Auth check
        if self.auth.validate_auth(auth_header).is_err() {
            return BeaconResult::Unauthorized;
        }

        // 5. Payload validation
        if validate_payload(payload).is_err() {
            return BeaconResult::InvalidPayload;
        }

        // 6. Persist
        if self.repo.upsert_beacon(payload).await.is_err() {
            return BeaconResult::InternalError;
        }

        BeaconResult::Success
    }
}

// ---------------------------------------------------------------------------
// StatsService
// ---------------------------------------------------------------------------

/// Service for retrieving aggregated statistics.
///
/// # Example
/// ```rust,ignore
/// // StatsService retrieves aggregated statistics.
/// // All methods check the rate limiter before delegating to the repository.
/// // See domain::services tests for mockall-based usage examples.
/// ```
pub struct StatsService<'a, R: BeaconRepository> {
    repo: R,
    rate_limiter: &'a dyn RateLimiter,
    stats_max_per_window: u32,
}

impl<'a, R: BeaconRepository> StatsService<'a, R> {
    /// Create a new StatsService with the given dependencies.
    pub fn new(repo: R, rate_limiter: &'a dyn RateLimiter, stats_max_per_window: u32) -> Self {
        Self { repo, rate_limiter, stats_max_per_window }
    }

    /// Get daily stats for the last 30 days.
    ///
    /// # Example
    /// ```rust,ignore
    /// // StatsService::get_stats() checks rate limit first before retrieving stats.
    /// // It returns a Result<StatsResponse, BeaconResult> with either the stats or a BeaconResult error.
    /// ```
    pub async fn get_stats(&self) -> Result<StatsResponse, BeaconResult> {
        if !self.rate_limiter.check(self.stats_max_per_window) {
            return Err(BeaconResult::RateLimited);
        }
        self.repo
            .get_daily_stats()
            .await
            .map(|stats| StatsResponse { stats })
            .map_err(|_| BeaconResult::InternalError)
    }

    /// Get summary statistics across all days.
    pub async fn get_summary(&self) -> Result<SummaryResponse, BeaconResult> {
        if !self.rate_limiter.check(self.stats_max_per_window) {
            return Err(BeaconResult::RateLimited);
        }
        self.repo.get_summary().await.map_err(|_| BeaconResult::InternalError)
    }

    /// Get Shields.io compatible badge data.
    pub async fn get_shield(&self) -> Result<ShieldResponse, BeaconResult> {
        if !self.rate_limiter.check(self.stats_max_per_window) {
            return Err(BeaconResult::RateLimited);
        }
        self.repo
            .get_total_instances()
            .await
            .map(|total| ShieldResponse {
                schema_version: 1,
                label: "NEXUS",
                message: format!("{} active", total),
                color: "blue",
                named_logo: "cloudflare",
            })
            .map_err(|_| BeaconResult::InternalError)
    }
}

// ---------------------------------------------------------------------------
// AggregationService
// ---------------------------------------------------------------------------

/// Service for scheduled data aggregation and cleanup.
///
/// # Example
/// ```rust,ignore
/// // AggregationService orchestrates: find dates needing aggregation → aggregate each → cleanup old data.
/// // See domain::services tests for mockall-based usage examples.
/// ```
pub struct AggregationService<R: BeaconRepository> {
    repo: R,
    beacon_retention_days: i64,
    stats_retention_days: i64,
}

impl<R: BeaconRepository> AggregationService<R> {
    /// Create a new AggregationService with the given retention periods.
    pub fn new(repo: R, beacon_retention_days: i64, stats_retention_days: i64) -> Self {
        Self { repo, beacon_retention_days, stats_retention_days }
    }

    /// Run aggregation for all dates that need it.
    ///
    /// # Example
    /// ```rust,ignore
    /// // AggregationService orchestrates: find dates needing aggregation → aggregate each → cleanup old data.
    /// // It finds dates needing aggregation and aggregates each.
    /// ```
    pub async fn run_aggregation(&self) -> Result<(), RepositoryError> {
        let dates = self.repo.get_dates_needing_aggregation().await?;
        for date in &dates {
            self.repo.aggregate_for_date(date).await?;
        }
        Ok(())
    }

    /// Delete old data beyond the retention windows.
    ///
    /// # Example
    /// ```rust,ignore
    /// // AggregationService::run_cleanup() deletes old data per retention policy.
    /// // It cleans up old beacon data and statistics based on the retention policy.
    /// ```
    pub async fn run_cleanup(&self) -> Result<(), RepositoryError> {
        self.repo.cleanup_old_data(self.beacon_retention_days, self.stats_retention_days).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{AuthProvider, BeaconRepository as BeaconRepositoryTrait};
    use crate::domain::rate_limit::RateLimiter;
    use crate::domain::types::{AuthError, DailyGlobalStats, RepositoryError};
    use mockall::predicate::*;

    // ---- mockall mocks ----

    mockall::mock! {
        pub TestRateLimiter {}
        impl RateLimiter for TestRateLimiter {
            fn check(&self, max_per_window: u32) -> bool;
        }
    }

    mockall::mock! {
        pub TestAuthProvider {}
        impl AuthProvider for TestAuthProvider {
            fn validate_auth(&self, auth_header: &str) -> Result<(), AuthError>;
        }
    }

    mockall::mock! {
        pub TestRepo {}
        impl BeaconRepositoryTrait for TestRepo {
            async fn upsert_beacon(&self, payload: &BeaconPayload) -> Result<(), RepositoryError>;
            async fn get_daily_stats(&self) -> Result<Vec<DailyGlobalStats>, RepositoryError>;
            async fn get_summary(&self) -> Result<SummaryResponse, RepositoryError>;
            async fn get_total_instances(&self) -> Result<i64, RepositoryError>;
            async fn get_dates_needing_aggregation(&self) -> Result<Vec<String>, RepositoryError>;
            async fn aggregate_for_date(&self, date: &str) -> Result<(), RepositoryError>;
            async fn cleanup_old_data(&self, beacon_retention_days: i64, stats_retention_days: i64) -> Result<(), RepositoryError>;
        }
    }

    /// Helper: valid BeaconPayload for tests.
    fn valid_payload() -> BeaconPayload {
        crate::domain::types::builders::BeaconPayloadBuilder::default().build()
    }

    // ---- BeaconService tests ----

    #[tokio::test]
    async fn receive_beacon_rate_limited_returns_rate_limited() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(false);

        let auth = MockTestAuthProvider::new();
        let repo = MockTestRepo::new();

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 100, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::RateLimited);
    }

    #[tokio::test]
    async fn receive_beacon_invalid_content_type_returns_invalid_content_type() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let auth = MockTestAuthProvider::new();
        let repo = MockTestRepo::new();

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result = svc.receive_beacon("text/plain", 100, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::InvalidContentType);
    }

    #[tokio::test]
    async fn receive_beacon_payload_too_large_returns_payload_too_large() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let auth = MockTestAuthProvider::new();
        let repo = MockTestRepo::new();

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 20_000, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::PayloadTooLarge);
    }

    #[tokio::test]
    async fn receive_beacon_unauthorized_returns_unauthorized() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut auth = MockTestAuthProvider::new();
        auth.expect_validate_auth().returning(|_| Err(AuthError::InvalidCredentials));

        let repo = MockTestRepo::new();

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 100, "Bearer wrong", &valid_payload()).await;
        assert_eq!(result, BeaconResult::Unauthorized);
    }

    #[tokio::test]
    async fn receive_beacon_invalid_payload_returns_invalid_payload() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut auth = MockTestAuthProvider::new();
        auth.expect_validate_auth().returning(|_| Ok(()));

        let repo = MockTestRepo::new();

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        // Empty instance_id is invalid
        let bad_payload = crate::domain::types::builders::BeaconPayloadBuilder::default()
            .instance_id(String::new())
            .build();
        let result =
            svc.receive_beacon("application/json", 100, "Bearer token", &bad_payload).await;
        assert_eq!(result, BeaconResult::InvalidPayload);
    }

    #[tokio::test]
    async fn receive_beacon_repo_failure_returns_internal_error() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut auth = MockTestAuthProvider::new();
        auth.expect_validate_auth().returning(|_| Ok(()));

        let mut repo = MockTestRepo::new();
        repo.expect_upsert_beacon()
            .returning(|_| Err(RepositoryError::DatabaseError("fail".to_string())));

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 100, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::InternalError);
    }

    #[tokio::test]
    async fn receive_beacon_success_returns_ok() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut auth = MockTestAuthProvider::new();
        auth.expect_validate_auth().returning(|_| Ok(()));

        let mut repo = MockTestRepo::new();
        repo.expect_upsert_beacon().returning(|_| Ok(()));

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 100, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::Success);
    }

    #[tokio::test]
    async fn receive_beacon_delegates_to_repository() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut auth = MockTestAuthProvider::new();
        auth.expect_validate_auth().returning(|_| Ok(()));

        // Verify repo is called exactly once with the correct payload
        let mut repo = MockTestRepo::new();
        repo.expect_upsert_beacon()
            .withf(|p| p.instance_id == "a".repeat(64))
            .times(1)
            .returning(|_| Ok(()));

        let svc = BeaconService::new(repo, auth, &rl, 100, 10_240);
        let result =
            svc.receive_beacon("application/json", 100, "Bearer token", &valid_payload()).await;
        assert_eq!(result, BeaconResult::Success);
    }

    // ---- StatsService tests ----

    #[tokio::test]
    async fn stats_service_rate_limited_returns_rate_limited() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(false);

        let repo = MockTestRepo::new();
        let svc = StatsService::new(repo, &rl, 200);

        let result = svc.get_stats().await;
        assert_eq!(result, Err(BeaconResult::RateLimited));
    }

    #[tokio::test]
    async fn stats_service_get_stats_delegates_to_repo() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut repo = MockTestRepo::new();
        repo.expect_get_daily_stats().returning(|| {
            Ok(vec![DailyGlobalStats {
                date: "2026-06-08".to_string(),
                total_instances: 5,
                total_requests: 500,
                total_unique_users: 3,
                models_used: "{}".to_string(),
                client_types: "{}".to_string(),
                avg_message_count: 4.0,
                tool_use_ratio: 0.3,
                versions: "{}".to_string(),
            }])
        });

        let svc = StatsService::new(repo, &rl, 200);
        let result = svc.get_stats().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().stats.len(), 1);
    }

    #[tokio::test]
    async fn stats_service_get_summary_delegates_to_repo() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut repo = MockTestRepo::new();
        repo.expect_get_summary().returning(|| {
            Ok(SummaryResponse {
                total_instances: 42,
                total_requests: 1000,
                total_unique_users: 20,
                days_active: 7,
            })
        });

        let svc = StatsService::new(repo, &rl, 200);
        let result = svc.get_summary().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().total_instances, 42);
    }

    #[tokio::test]
    async fn stats_service_get_shield_delegates_to_repo() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut repo = MockTestRepo::new();
        repo.expect_get_total_instances().returning(|| Ok(99));

        let svc = StatsService::new(repo, &rl, 200);
        let result = svc.get_shield().await;
        assert!(result.is_ok());
        let shield = result.unwrap();
        assert_eq!(shield.label, "NEXUS");
        assert_eq!(shield.message, "99 active");
    }

    #[tokio::test]
    async fn stats_service_get_stats_repo_error_returns_internal_error() {
        let mut rl = MockTestRateLimiter::new();
        rl.expect_check().return_const(true);

        let mut repo = MockTestRepo::new();
        repo.expect_get_daily_stats()
            .returning(|| Err(RepositoryError::DatabaseError("fail".to_string())));

        let svc = StatsService::new(repo, &rl, 200);
        let result = svc.get_stats().await;
        assert_eq!(result, Err(BeaconResult::InternalError));
    }

    // ---- AggregationService tests ----

    #[tokio::test]
    async fn aggregation_service_run_aggregation_orchestrates_dates() {
        let mut repo = MockTestRepo::new();
        repo.expect_get_dates_needing_aggregation()
            .returning(|| Ok(vec!["2026-06-07".to_string(), "2026-06-08".to_string()]));
        repo.expect_aggregate_for_date().with(eq("2026-06-07")).returning(|_| Ok(()));
        repo.expect_aggregate_for_date().with(eq("2026-06-08")).returning(|_| Ok(()));

        let svc = AggregationService::new(repo, 90, 365);
        let result = svc.run_aggregation().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn aggregation_service_run_aggregation_stops_on_first_error() {
        let mut repo = MockTestRepo::new();
        repo.expect_get_dates_needing_aggregation()
            .returning(|| Ok(vec!["2026-06-07".to_string(), "2026-06-08".to_string()]));
        repo.expect_aggregate_for_date()
            .with(eq("2026-06-07"))
            .returning(|_| Err(RepositoryError::DatabaseError("fail".to_string())));
        // aggregate_for_date("2026-06-08") should NOT be called

        let svc = AggregationService::new(repo, 90, 365);
        let result = svc.run_aggregation().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn aggregation_service_run_aggregation_empty_dates_is_ok() {
        let mut repo = MockTestRepo::new();
        repo.expect_get_dates_needing_aggregation().returning(|| Ok(Vec::new()));

        let svc = AggregationService::new(repo, 90, 365);
        let result = svc.run_aggregation().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn aggregation_service_run_cleanup_calls_repo_with_correct_retention() {
        let mut repo = MockTestRepo::new();
        repo.expect_cleanup_old_data().with(eq(90), eq(365)).times(1).returning(|_, _| Ok(()));

        let svc = AggregationService::new(repo, 90, 365);
        let result = svc.run_cleanup().await;
        assert!(result.is_ok());
    }
}
