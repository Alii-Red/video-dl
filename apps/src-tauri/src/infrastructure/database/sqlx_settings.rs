use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use crate::application::error::ApplicationError;
use crate::application::ports::SettingsRepository;
use crate::domain::value_objects::Settings;

/// Settings persisted as a single JSON row (`key = 'app'`) in the `settings`
/// table. Shares the SQLite file with the download repository.
pub struct SqlxSettingsRepository {
    pool: SqlitePool,
}

const KEY: &str = "app";

impl SqlxSettingsRepository {
    /// Build from an existing pool (the app opens the DB once and shares it).
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SettingsRepository for SqlxSettingsRepository {
    async fn load(&self) -> Result<Settings, ApplicationError> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(KEY)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;

        match row {
            Some(row) => {
                let json: String = row
                    .try_get("value")
                    .map_err(|e| ApplicationError::Repository(e.to_string()))?;
                serde_json::from_str(&json)
                    .map_err(|e| ApplicationError::Repository(format!("bad settings json: {e}")))
            }
            None => Ok(Settings::default()),
        }
    }

    async fn save(&self, settings: &Settings) -> Result<(), ApplicationError> {
        let json = serde_json::to_string(settings)
            .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(KEY)
        .bind(json)
        .execute(&self.pool)
        .await
        .map_err(|e| ApplicationError::Repository(e.to_string()))?;
        Ok(())
    }
}
