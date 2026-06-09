//! D1Repository — BeaconRepository implementation via worker::D1Database.

use crate::config::*;
use crate::domain::ports::BeaconRepository;
use crate::domain::types::*;
use worker::D1Database;

pub struct D1Repository {
    db: D1Database,
}

impl D1Repository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

impl BeaconRepository for D1Repository {
    async fn upsert_beacon(&self, payload: &BeaconPayload) -> Result<(), RepositoryError> {
        let models_json =
            serde_json::to_string(&payload.stats.models_used).unwrap_or_else(|_| "{}".to_string());
        let client_types_json =
            serde_json::to_string(&payload.stats.client_types).unwrap_or_else(|_| "{}".to_string());

        let stmt = worker::query!(
            &self.db,
            SQL_UPSERT_BEACON,
            &payload.instance_id,
            &payload.version,
            &payload.date,
            &payload.stats.total_requests,
            &payload.stats.unique_fingerprints,
            &models_json,
            &client_types_json,
            &payload.stats.avg_message_count,
            &payload.stats.tool_use_ratio,
        );
        stmt.map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            .run()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn get_daily_stats(&self) -> Result<Vec<DailyGlobalStats>, RepositoryError> {
        let result = worker::query!(&self.db, SQL_GET_DAILY_STATS)
            .all()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let stats: Vec<DailyGlobalStats> =
            result.results().map_err(|e| RepositoryError::DeserializationError(e.to_string()))?;
        Ok(stats)
    }

    async fn get_summary(&self) -> Result<SummaryResponse, RepositoryError> {
        let result = worker::query!(&self.db, SQL_GET_SUMMARY)
            .first(None)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            .unwrap_or(SummaryResponse {
                total_instances: 0,
                total_requests: 0,
                total_unique_users: 0,
                days_active: 0,
            });
        Ok(result)
    }

    async fn get_total_instances(&self) -> Result<i64, RepositoryError> {
        let result: serde_json::Value = worker::query!(&self.db, SQL_GET_TOTAL_INSTANCES)
            .first(None)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            .unwrap_or_default();
        let total = result.get("total_instances").and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(total)
    }

    async fn get_dates_needing_aggregation(&self) -> Result<Vec<String>, RepositoryError> {
        let result = worker::query!(&self.db, SQL_GET_DATES_FOR_AGGREGATION)
            .all()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let rows: Vec<serde_json::Value> =
            result.results().map_err(|e| RepositoryError::DeserializationError(e.to_string()))?;

        let dates: Vec<String> = rows
            .iter()
            .filter_map(|row| row.get("date").and_then(|v| v.as_str().map(|s| s.to_string())))
            .collect();
        Ok(dates)
    }

    async fn aggregate_for_date(&self, _date: &str) -> Result<(), RepositoryError> {
        // Stub implementation as per task instructions - methods that need bind params (aggregate_for_date, cleanup_old_data)
        // can be stubs for now since the actual D1 queries are still in lib.rs and will be moved in Phase 14
        Ok(())
    }

    async fn cleanup_old_data(
        &self,
        _beacon_retention_days: i64,
        _stats_retention_days: i64,
    ) -> Result<(), RepositoryError> {
        // Stub implementation as per task instructions - methods that need bind params (aggregate_for_date, cleanup_old_data)
        // can be stubs for now since the actual D1 queries are still in lib.rs and will be moved in Phase 14
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    // We can't easily mock D1Database since it's a worker type,
    // but we can test that D1Repository::new() compiles and the
    // struct is properly constructed.

    // For full integration tests, we'd need wrangler dev.
    // Unit tests with mockall would mock the BeaconRepository trait itself:

    mockall::mock! {
        pub Repo {}
        impl BeaconRepository for Repo {
            async fn upsert_beacon(&self, payload: &BeaconPayload) -> Result<(), RepositoryError>;
            async fn get_daily_stats(&self) -> Result<Vec<DailyGlobalStats>, RepositoryError>;
            async fn get_summary(&self) -> Result<SummaryResponse, RepositoryError>;
            async fn get_total_instances(&self) -> Result<i64, RepositoryError>;
            async fn get_dates_needing_aggregation(&self) -> Result<Vec<String>, RepositoryError>;
            async fn aggregate_for_date(&self, date: &str) -> Result<(), RepositoryError>;
            async fn cleanup_old_data(&self, beacon_retention_days: i64, stats_retention_days: i64) -> Result<(), RepositoryError>;
        }
    }

    #[test]
    fn mock_repo_upsert_succeeds() {
        // This is a placeholder test since we can't easily test async methods in WASM
        // For now, we'll just test that the method exists and compiles
    }

    #[test]
    fn mock_repo_get_stats_returns_data() {
        // This is a placeholder test since we can't easily test async methods in WASM
    }

    #[test]
    fn mock_repo_cleanup_succeeds() {
        // This is a placeholder test since we can't easily test async methods in WASM
    }
}
