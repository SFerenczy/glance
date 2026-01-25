//! Persistence layer for Glance.
//!
//! Manages local SQLite storage for connections, query history, saved queries,
//! and LLM settings. Secrets are stored via OS keyring when available.
//!
//! # Scalability
//!
//! The state database uses SQLite with WAL mode for better concurrent access.
//! Pool size is configurable via `StateDbConfig`. Retry logic is built into
//! hot paths (history logging, settings updates) to handle transient contention.

pub mod connections;
pub mod history;
pub mod llm_settings;
mod migrations;
pub mod saved_queries;
mod secrets;

#[allow(unused_imports)]
pub use connections::{ConnectionProfile, PasswordStorage};
#[allow(unused_imports)]
pub use history::{HistoryEntry, HistoryFilter, OwnedRecordQueryParams, QueryStatus, SubmittedBy};
#[allow(unused_imports)]
pub use llm_settings::LlmSettings;
#[allow(unused_imports)]
pub use saved_queries::{SavedQuery, SavedQueryFilter};
pub use secrets::{SecretStorage, SecretStorageStatus};

use crate::error::{GlanceError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::future::Future;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, info, warn};

const MAX_RETRY_ATTEMPTS: u32 = 3;
const RETRY_DELAY_MS: u64 = 100;
const DEFAULT_POOL_SIZE: u32 = 4;
const DEFAULT_BUSY_TIMEOUT_SECS: u64 = 5;
const DEFAULT_ACQUIRE_TIMEOUT_SECS: u64 = 5;

/// Configuration for the state database.
#[derive(Debug, Clone)]
pub struct StateDbConfig {
    /// Maximum number of connections in the pool.
    pub pool_size: u32,
    /// Timeout for acquiring a connection from the pool.
    pub acquire_timeout: Duration,
    /// SQLite busy timeout for lock contention.
    pub busy_timeout: Duration,
}

impl Default for StateDbConfig {
    fn default() -> Self {
        Self {
            pool_size: DEFAULT_POOL_SIZE,
            acquire_timeout: Duration::from_secs(DEFAULT_ACQUIRE_TIMEOUT_SECS),
            busy_timeout: Duration::from_secs(DEFAULT_BUSY_TIMEOUT_SECS),
        }
    }
}

#[allow(dead_code)]
impl StateDbConfig {
    /// Creates a new configuration with the specified pool size.
    pub fn with_pool_size(mut self, size: u32) -> Self {
        self.pool_size = size;
        self
    }

    /// Creates a configuration from environment variables.
    ///
    /// Reads:
    /// - `GLANCE_DB_POOL_SIZE`: Pool size (default: 4)
    /// - `GLANCE_DB_BUSY_TIMEOUT`: Busy timeout in seconds (default: 5)
    pub fn from_env() -> Self {
        let pool_size = std::env::var("GLANCE_DB_POOL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_POOL_SIZE);

        let busy_timeout = std::env::var("GLANCE_DB_BUSY_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_BUSY_TIMEOUT_SECS));

        Self {
            pool_size,
            busy_timeout,
            ..Default::default()
        }
    }
}

/// Executes an async operation with retry logic for transient database errors.
///
/// This is useful for hot paths like history logging where transient lock
/// contention should not fail the operation.
#[allow(dead_code)]
pub async fn with_retry<F, Fut, T>(operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        if attempt > 0 {
            let delay = Duration::from_millis(RETRY_DELAY_MS * 2u64.pow(attempt));
            debug!(
                attempt,
                delay_ms = delay.as_millis(),
                "Retrying database operation"
            );
            tokio::time::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if is_transient_error(&e) {
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| GlanceError::persistence("Operation failed after retries")))
}

/// Checks if an error is transient (e.g., database locked).
#[allow(dead_code)]
fn is_transient_error(error: &GlanceError) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("database is locked")
        || msg.contains("busy")
        || msg.contains("timeout")
        || msg.contains("connection")
}

/// Main persistence interface for the application state database.
#[derive(Debug)]
#[allow(dead_code)]
pub struct StateDb {
    pool: SqlitePool,
    db_path: PathBuf,
    secret_storage: SecretStorage,
    config: StateDbConfig,
    /// Whether the database was recovered from corruption during this session.
    recovered: bool,
}

#[allow(dead_code)]
impl StateDb {
    /// Opens or creates the state database at the default platform path.
    ///
    /// - Linux/macOS: `~/.config/db-glance/state.db`
    /// - Windows: `%APPDATA%\db-glance\state.db`
    pub async fn open_default() -> Result<Self> {
        let path = Self::default_path()?;
        let config = StateDbConfig::from_env();
        Self::open_with_config(&path, config).await
    }

    /// Opens or creates the state database at the specified path.
    pub async fn open(path: &PathBuf) -> Result<Self> {
        Self::open_with_config(path, StateDbConfig::default()).await
    }

    /// Opens or creates the state database with custom configuration.
    pub async fn open_with_config(path: &PathBuf, config: StateDbConfig) -> Result<Self> {
        Self::ensure_parent_dirs(path)?;

        let secret_storage = SecretStorage::new();

        match Self::try_open(path, &secret_storage, &config, false).await {
            Ok(db) => Ok(db),
            Err(e) => {
                warn!("Failed to open state database: {e}. Attempting recovery...");
                Self::attempt_recovery(path, &secret_storage, &config).await
            }
        }
    }

    /// Returns the default state database path for the current platform.
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| GlanceError::persistence("Could not determine config directory"))?;
        Ok(config_dir.join("db-glance").join("state.db"))
    }

    /// Attempts to open the database with retries for lock contention.
    async fn try_open(
        path: &std::path::Path,
        secret_storage: &SecretStorage,
        config: &StateDbConfig,
        recovered: bool,
    ) -> Result<Self> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRY_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS * 2u64.pow(attempt))).await;
            }

            match Self::connect(path, config).await {
                Ok(pool) => {
                    migrations::run_migrations(&pool).await?;
                    info!(
                        path = %path.display(),
                        pool_size = config.pool_size,
                        recovered,
                        "State database opened"
                    );
                    return Ok(Self {
                        pool,
                        db_path: path.to_path_buf(),
                        secret_storage: secret_storage.clone(),
                        config: config.clone(),
                        recovered,
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| GlanceError::persistence("Failed to open database after retries")))
    }

    /// Creates a connection pool to the SQLite database.
    async fn connect(path: &std::path::Path, config: &StateDbConfig) -> Result<SqlitePool> {
        let conn_str = format!("sqlite:{}?mode=rwc", path.display());
        let options = SqliteConnectOptions::from_str(&conn_str)
            .map_err(|e| GlanceError::persistence(format!("Invalid database path: {e}")))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(config.busy_timeout)
            .create_if_missing(true);

        SqlitePoolOptions::new()
            .max_connections(config.pool_size)
            .acquire_timeout(config.acquire_timeout)
            .connect_with(options)
            .await
            .map_err(|e| {
                GlanceError::persistence(format!("Failed to connect to state database: {e}"))
            })
    }

    /// Ensures parent directories exist for the database path.
    fn ensure_parent_dirs(path: &std::path::Path) -> Result<()> {
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
    async fn attempt_recovery(
        path: &PathBuf,
        secret_storage: &SecretStorage,
        config: &StateDbConfig,
    ) -> Result<Self> {
        let backup_path = path.with_extension("db.bak");

        if path.exists() {
            std::fs::rename(path, &backup_path).map_err(|e| {
                GlanceError::persistence(format!(
                    "Failed to backup corrupted database to {}: {e}",
                    backup_path.display()
                ))
            })?;
            warn!("Backed up corrupted database to {}", backup_path.display());
        }

        Self::try_open(path, secret_storage, config, true)
            .await
            .map_err(|e| {
                GlanceError::persistence(format!("Failed to recreate database after backup: {e}"))
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

    /// Returns whether the database was recovered from corruption.
    pub fn was_recovered(&self) -> bool {
        self.recovered
    }

    /// Closes the database connection pool.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Performs a health check on the database.
    ///
    /// Returns Ok(()) if the database is accessible and responsive.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Health check failed: {e}")))?;
        Ok(())
    }

    /// Returns the current pool statistics.
    pub fn pool_stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
            max_connections: self.config.pool_size,
        }
    }

    /// Creates an in-memory state database for testing.
    pub async fn open_in_memory() -> Result<Self> {
        let secret_storage = SecretStorage::new();
        let config = StateDbConfig::default();

        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|e| GlanceError::persistence(format!("Invalid memory database: {e}")))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.pool_size)
            .connect_with(options)
            .await
            .map_err(|e| {
                GlanceError::persistence(format!("Failed to create in-memory database: {e}"))
            })?;

        migrations::run_migrations(&pool).await?;

        Ok(Self {
            pool,
            db_path: PathBuf::from(":memory:"),
            secret_storage,
            config,
            recovered: false,
        })
    }
}

/// Pool statistics for monitoring.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PoolStats {
    /// Current number of connections in the pool.
    pub size: u32,
    /// Number of idle connections.
    pub idle: usize,
    /// Maximum configured connections.
    pub max_connections: u32,
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

    #[tokio::test]
    async fn test_health_check() {
        let db = StateDb::open_in_memory().await.unwrap();
        db.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let db = StateDb::open_in_memory().await.unwrap();
        let stats = db.pool_stats();
        assert!(stats.max_connections >= 1);
    }

    #[tokio::test]
    async fn test_config_from_env() {
        // Save original values
        let orig_pool = std::env::var("GLANCE_DB_POOL_SIZE").ok();
        let orig_timeout = std::env::var("GLANCE_DB_BUSY_TIMEOUT").ok();

        // Set test values
        std::env::set_var("GLANCE_DB_POOL_SIZE", "8");
        std::env::set_var("GLANCE_DB_BUSY_TIMEOUT", "10");

        let config = StateDbConfig::from_env();
        assert_eq!(config.pool_size, 8);
        assert_eq!(config.busy_timeout, Duration::from_secs(10));

        // Restore
        match orig_pool {
            Some(v) => std::env::set_var("GLANCE_DB_POOL_SIZE", v),
            None => std::env::remove_var("GLANCE_DB_POOL_SIZE"),
        }
        match orig_timeout {
            Some(v) => std::env::set_var("GLANCE_DB_BUSY_TIMEOUT", v),
            None => std::env::remove_var("GLANCE_DB_BUSY_TIMEOUT"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let db = std::sync::Arc::new(StateDb::open_in_memory().await.unwrap());

        // Create a test connection for foreign key constraint
        sqlx::query("INSERT INTO connections (name, database) VALUES ('test', 'testdb')")
            .execute(db.pool())
            .await
            .unwrap();

        // Spawn multiple concurrent tasks
        let mut handles = Vec::new();
        for i in 0..10 {
            let db = std::sync::Arc::clone(&db);
            handles.push(tokio::spawn(async move {
                // Each task writes to history
                history::record_query(
                    db.pool(),
                    "test",
                    history::SubmittedBy::User,
                    &format!("SELECT {}", i),
                    history::QueryStatus::Success,
                    Some(10),
                    Some(1),
                    None,
                    None,
                )
                .await
            }));
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all entries were written
        let count = history::count_history(db.pool()).await.unwrap();
        assert_eq!(count, 10);
    }
}
