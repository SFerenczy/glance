//! Schema versioning and migrations for the state database.
//!
//! Manages database schema evolution with forward-only migrations.

use crate::error::{GlanceError, Result};
use sqlx::sqlite::SqlitePool;
use tracing::info;

const CURRENT_VERSION: i32 = 2;

/// Runs all pending migrations on the database.
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    ensure_schema_versions_table(pool).await?;

    let current = get_current_version(pool).await?;

    // Check if database is newer than code
    if current > CURRENT_VERSION {
        return Err(GlanceError::persistence(format!(
            "Database schema version ({}) is newer than supported version ({}). \
             Please upgrade Glance to the latest version.",
            current, CURRENT_VERSION
        )));
    }

    if current < CURRENT_VERSION {
        info!(
            "Migrating state database from version {} to {}",
            current, CURRENT_VERSION
        );
        run_pending_migrations(pool, current).await?;
    }

    Ok(())
}

/// Ensures the schema_versions table exists.
async fn ensure_schema_versions_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_versions (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        GlanceError::persistence(format!("Failed to create schema_versions table: {e}"))
    })?;

    Ok(())
}

/// Gets the current schema version.
async fn get_current_version(pool: &SqlitePool) -> Result<i32> {
    let row: Option<(i32,)> = sqlx::query_as("SELECT MAX(version) FROM schema_versions")
        .fetch_optional(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to get schema version: {e}")))?;

    Ok(row.map(|(v,)| v).unwrap_or(0))
}

/// Runs migrations from the current version to the target version.
async fn run_pending_migrations(pool: &SqlitePool, from_version: i32) -> Result<()> {
    for version in (from_version + 1)..=CURRENT_VERSION {
        run_migration(pool, version).await?;
        record_version(pool, version).await?;
        info!("Applied migration v{}", version);
    }
    Ok(())
}

/// Records a completed migration version.
async fn record_version(pool: &SqlitePool, version: i32) -> Result<()> {
    sqlx::query("INSERT INTO schema_versions (version) VALUES (?)")
        .bind(version)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to record migration: {e}")))?;
    Ok(())
}

/// Runs a specific migration version.
async fn run_migration(pool: &SqlitePool, version: i32) -> Result<()> {
    match version {
        1 => migration_v1(pool).await,
        2 => migration_v2(pool).await,
        _ => Err(GlanceError::persistence(format!(
            "Unknown migration version: {version}"
        ))),
    }
}

/// Migration v1: Initial schema with all tables.
async fn migration_v1(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS connections (
            name TEXT PRIMARY KEY,
            database TEXT NOT NULL,
            host TEXT,
            port INTEGER NOT NULL DEFAULT 5432,
            username TEXT,
            sslmode TEXT,
            extras TEXT,
            password_storage TEXT NOT NULL DEFAULT 'none',
            password_plaintext TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_used_at TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create connections table: {e}")))?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS query_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            connection_name TEXT NOT NULL,
            submitted_by TEXT NOT NULL CHECK (submitted_by IN ('user', 'llm')),
            sql TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('success', 'error', 'cancelled')),
            execution_time_ms INTEGER,
            row_count INTEGER,
            error_message TEXT,
            saved_query_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (connection_name) REFERENCES connections(name) ON DELETE CASCADE,
            FOREIGN KEY (saved_query_id) REFERENCES saved_queries(id) ON DELETE SET NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create query_history table: {e}")))?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_query_history_connection 
        ON query_history(connection_name)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create history index: {e}")))?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_query_history_created 
        ON query_history(created_at)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create history index: {e}")))?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS saved_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            sql TEXT NOT NULL,
            description TEXT,
            connection_name TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_used_at TEXT,
            usage_count INTEGER NOT NULL DEFAULT 0,
            UNIQUE(name, connection_name),
            FOREIGN KEY (connection_name) REFERENCES connections(name) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create saved_queries table: {e}")))?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_saved_queries_connection 
        ON saved_queries(connection_name)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create saved_queries index: {e}")))?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS saved_query_tags (
            saved_query_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (saved_query_id, tag),
            FOREIGN KEY (saved_query_id) REFERENCES saved_queries(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        GlanceError::persistence(format!("Failed to create saved_query_tags table: {e}"))
    })?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_saved_query_tags_tag 
        ON saved_query_tags(tag)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create tags index: {e}")))?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL DEFAULT 'gpt-5',
            api_key_storage TEXT NOT NULL DEFAULT 'none',
            api_key_plaintext TEXT,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to create llm_settings table: {e}")))?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO llm_settings (id, provider, model) 
        VALUES (1, 'openai', 'gpt-5')
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to initialize llm_settings: {e}")))?;

    Ok(())
}

/// Migration v2: Add backend column to connections table.
async fn migration_v2(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        ALTER TABLE connections ADD COLUMN backend TEXT NOT NULL DEFAULT 'postgres'
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to add backend column: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_migrations_run_successfully() {
        let pool = test_pool().await;
        run_migrations(&pool).await.unwrap();

        let version = get_current_version(&pool).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[tokio::test]
    async fn test_migrations_are_idempotent() {
        let pool = test_pool().await;

        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let version = get_current_version(&pool).await.unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[tokio::test]
    async fn test_tables_created() {
        let pool = test_pool().await;
        run_migrations(&pool).await.unwrap();

        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();
        assert!(table_names.contains(&"connections"));
        assert!(table_names.contains(&"query_history"));
        assert!(table_names.contains(&"saved_queries"));
        assert!(table_names.contains(&"saved_query_tags"));
        assert!(table_names.contains(&"llm_settings"));
        assert!(table_names.contains(&"schema_versions"));
    }
}
