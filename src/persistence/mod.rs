//! Persistence layer for Glance.
//!
//! Manages local SQLite storage for connections, query history, saved queries,
//! and LLM settings. Secrets are stored via OS keyring when available.

mod connections;
mod history;
mod llm_settings;
mod migrations;
mod saved_queries;
mod secrets;

pub use connections::{ConnectionProfile, ConnectionProfileRow};
pub use history::{HistoryEntry, HistoryFilter, SubmittedBy, QueryStatus};
pub use llm_settings::LlmSettings;
pub use saved_queries::{SavedQuery, SavedQueryFilter, SavedQueryTag};
pub use secrets::{SecretStorage, SecretStorageStatus};

use crate::error::{GlanceError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, warn};

const MAX_RETRY_ATTEMPTS: u32 = 3;
const RETRY_DELAY_MS: u64 = 100;

/// Main persistence interface for the application state database.
pub struct StateDb {
    pool: SqlitePool,
    db_path: PathBuf,
    secret_storage: SecretStorage,
}

impl StateDb {
    /// Opens or creates the state database at the default platform path.
    ///
    /// - Linux/macOS: `~/.config/db-glance/state.db`
    /// - Windows: `%APPDATA%\db-glance\state.db`
    pub async fn open_default() -> Result<Self> {
        let path = Self::default_path()?;
        Self::open(&path).await
    }

    /// Opens or creates the state database at the specified path.
    pub async fn open(path: &PathBuf) -> Result<Self> {
        Self::ensure_parent_dirs(path)?;

        let secret_storage = SecretStorage::new();

        match Self::try_open(path, &secret_storage).await {
            Ok(db) => Ok(db),
            Err(e) => {
                warn!("Failed to open state database: {e}. Attempting recovery...");
                Self::attempt_recovery(path, &secret_storage).await
            }
        }
    }

    /// Returns the default state database path for the current platform.
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| {
            GlanceError::persistence("Could not determine config directory")
        })?;
        Ok(config_dir.join("db-glance").join("state.db"))
    }

    /// Attempts to open the database with retries for lock contention.
    async fn try_open(path: &PathBuf, secret_storage: &SecretStorage) -> Result<Self> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRY_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(
                    RETRY_DELAY_MS * 2u64.pow(attempt),
                ))
                .await;
            }

            match Self::connect(path).await {
                Ok(pool) => {
                    migrations::run_migrations(&pool).await?;
                    info!("State database opened at {}", path.display());
                    return Ok(Self {
                        pool,
                        db_path: path.clone(),
                        secret_storage: secret_storage.clone(),
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            GlanceError::persistence("Failed to open database after retries")
        }))
    }

    /// Creates a connection pool to the SQLite database.
    async fn connect(path: &PathBuf) -> Result<SqlitePool> {
        let conn_str = format!("sqlite:{}?mode=rwc", path.display());
        let options = SqliteConnectOptions::from_str(&conn_str)
            .map_err(|e| GlanceError::persistence(format!("Invalid database path: {e}")))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .create_if_missing(true);

        SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to connect to state database: {e}")))
    }

    /// Ensures parent directories exist for the database path.
    fn ensure_parent_dirs(path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GlanceError::persistence(format!(
                    "Failed to create config directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        Ok(())
    }

    /// Attempts to recover from a corrupted database by backing up and recreating.
    async fn attempt_recovery(path: &PathBuf, secret_storage: &SecretStorage) -> Result<Self> {
        let backup_path = path.with_extension("db.bak");

        if path.exists() {
            std::fs::rename(path, &backup_path).map_err(|e| {
                GlanceError::persistence(format!(
                    "Failed to backup corrupted database to {}: {e}",
                    backup_path.display()
                ))
            })?;
            warn!(
                "Backed up corrupted database to {}",
                backup_path.display()
            );
        }

        Self::try_open(path, secret_storage).await.map_err(|e| {
            GlanceError::persistence(format!(
                "Failed to recreate database after backup: {e}"
            ))
        })
    }

    /// Returns the path to the state database.
    pub fn path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Returns the secret storage interface.
    pub fn secrets(&self) -> &SecretStorage {
        &self.secret_storage
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Returns the status of secure storage availability.
    pub fn secret_storage_status(&self) -> SecretStorageStatus {
        self.secret_storage.status()
    }

    /// Closes the database connection pool.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_open_creates_database() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_state.db");

        let db = StateDb::open(&path).await.unwrap();
        assert!(path.exists());
        db.close().await;
    }

    #[tokio::test]
    async fn test_open_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("dirs").join("state.db");

        let db = StateDb::open(&path).await.unwrap();
        assert!(path.exists());
        db.close().await;
    }
}
