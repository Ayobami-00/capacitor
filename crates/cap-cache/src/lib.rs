use std::path::Path;

use cap_core::OfferObservation;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] sqlx::Error),
    #[error("failed to serialize observation: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct ObservationCache {
    pool: SqlitePool,
}

#[derive(Debug, PartialEq)]
pub struct CacheStats {
    pub total: i64,
    pub pending: i64,
    pub synced: i64,
}

impl ObservationCache {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        let cache = Self { pool };
        cache.initialize().await?;
        Ok(cache)
    }

    async fn initialize(&self) -> Result<(), CacheError> {
        self.pool
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS observations (
                    id TEXT PRIMARY KEY NOT NULL,
                    cache_key TEXT UNIQUE NOT NULL,
                    observed_at TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    provider_offer_id TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    synced_at TEXT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                "#,
            )
            .await?;

        self.pool
            .execute(
                r#"
                CREATE INDEX IF NOT EXISTS observations_pending_idx
                ON observations (synced_at, observed_at);
                "#,
            )
            .await?;

        Ok(())
    }

    pub async fn insert_observations(
        &self,
        observations: &[OfferObservation],
    ) -> Result<u64, CacheError> {
        let mut inserted = 0;

        for observation in observations {
            let payload_json = serde_json::to_string(observation)?;
            let result = sqlx::query(
                r#"
                INSERT OR IGNORE INTO observations
                    (id, cache_key, observed_at, provider, provider_offer_id, payload_json)
                VALUES
                    (?1, ?2, ?3, ?4, ?5, ?6);
                "#,
            )
            .bind(observation.observation_id.to_string())
            .bind(observation.cache_key())
            .bind(observation.observed_at.to_rfc3339())
            .bind(&observation.provider)
            .bind(&observation.provider_offer_id)
            .bind(payload_json)
            .execute(&self.pool)
            .await?;

            inserted += result.rows_affected();
        }

        Ok(inserted)
    }

    pub async fn pending_observations(
        &self,
        limit: i64,
    ) -> Result<Vec<OfferObservation>, CacheError> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT payload_json
            FROM observations
            WHERE synced_at IS NULL
            ORDER BY observed_at ASC
            LIMIT ?1;
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|payload| serde_json::from_str(&payload).map_err(CacheError::from))
            .collect()
    }

    pub async fn mark_synced(
        &self,
        observations: &[OfferObservation],
        synced_at: DateTime<Utc>,
    ) -> Result<u64, CacheError> {
        let mut updated = 0;

        for observation in observations {
            let result = sqlx::query(
                r#"
                UPDATE observations
                SET synced_at = ?1
                WHERE id = ?2;
                "#,
            )
            .bind(synced_at.to_rfc3339())
            .bind(observation.observation_id.to_string())
            .execute(&self.pool)
            .await?;

            updated += result.rows_affected();
        }

        Ok(updated)
    }

    pub async fn stats(&self) -> Result<CacheStats, CacheError> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM observations")
            .fetch_one(&self.pool)
            .await?;
        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM observations WHERE synced_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        let synced = total - pending;

        Ok(CacheStats {
            total,
            pending,
            synced,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_core::OfferObservation;
    use chrono::Utc;
    use uuid::Uuid;

    fn observation() -> OfferObservation {
        OfferObservation {
            observation_id: Uuid::new_v4(),
            observed_at: Utc::now(),
            provider: "vast".to_string(),
            provider_offer_id: "123".to_string(),
            gpu_name: "H100 SXM".to_string(),
            num_gpus: 1,
            gpu_ram_gb: Some(80.0),
            price_usd_per_hour: 2.9,
            reliability_score: Some(0.99),
            verified: true,
            rentable: true,
            region: Some("US".to_string()),
            host_id_hash: Some("hash".to_string()),
            raw_provider_payload: serde_json::json!({ "id": 123 }),
        }
    }

    #[tokio::test]
    async fn inserts_and_marks_observations_synced() {
        let tempdir = tempfile::tempdir().unwrap();
        let cache = ObservationCache::connect(tempdir.path().join("cap.db"))
            .await
            .unwrap();
        let observation = observation();

        assert_eq!(
            cache
                .insert_observations(std::slice::from_ref(&observation))
                .await
                .unwrap(),
            1
        );
        assert_eq!(cache.pending_observations(10).await.unwrap().len(), 1);

        cache
            .mark_synced(std::slice::from_ref(&observation), Utc::now())
            .await
            .unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.synced, 1);
    }

    #[tokio::test]
    async fn dedupes_same_cache_key() {
        let tempdir = tempfile::tempdir().unwrap();
        let cache = ObservationCache::connect(tempdir.path().join("cap.db"))
            .await
            .unwrap();
        let observation = observation();

        cache
            .insert_observations(std::slice::from_ref(&observation))
            .await
            .unwrap();
        cache
            .insert_observations(std::slice::from_ref(&observation))
            .await
            .unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 1);
    }
}
