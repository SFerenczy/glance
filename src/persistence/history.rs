//! Query history persistence.
//!
//! Records and retrieves executed queries with retention management.

use crate::error::{GlanceError, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;

const MAX_HISTORY_ENTRIES: i64 = 5000;
const MAX_HISTORY_DAYS: i64 = 90;

/// Who submitted the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubmittedBy {
    User,
    Llm,
}

impl SubmittedBy {
    fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Llm => "llm",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "llm" => Self::Llm,
            _ => Self::User,
        }
    }
}

/// Query execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryStatus {
    Success,
    Error,
    Cancelled,
}

impl QueryStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "error" => Self::Error,
            "cancelled" => Self::Cancelled,
            _ => Self::Success,
        }
    }
}

/// A query history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub connection_name: String,
    pub submitted_by: SubmittedBy,
    pub sql: String,
    pub status: QueryStatus,
    pub execution_time_ms: Option<i64>,
    pub row_count: Option<i64>,
    pub error_message: Option<String>,
    pub saved_query_id: Option<i64>,
    pub created_at: String,
}

/// Raw database row for history entry.
#[derive(Debug, Clone, FromRow)]
struct HistoryEntryRow {
    id: i64,
    connection_name: String,
    submitted_by: String,
    sql: String,
    status: String,
    execution_time_ms: Option<i64>,
    row_count: Option<i64>,
    error_message: Option<String>,
    saved_query_id: Option<i64>,
    created_at: String,
}

impl From<HistoryEntryRow> for HistoryEntry {
    fn from(row: HistoryEntryRow) -> Self {
        Self {
            id: row.id,
            connection_name: row.connection_name,
            submitted_by: SubmittedBy::from_str(&row.submitted_by),
            sql: row.sql,
            status: QueryStatus::from_str(&row.status),
            execution_time_ms: row.execution_time_ms,
            row_count: row.row_count,
            error_message: row.error_message,
            saved_query_id: row.saved_query_id,
            created_at: row.created_at,
        }
    }
}

/// Filter options for querying history.
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub connection_name: Option<String>,
    pub text_search: Option<String>,
    pub since_days: Option<i64>,
    pub limit: Option<i64>,
}

/// Records a new query execution in history.
pub async fn record_query(
    pool: &SqlitePool,
    connection_name: &str,
    submitted_by: SubmittedBy,
    sql: &str,
    status: QueryStatus,
    execution_time_ms: Option<i64>,
    row_count: Option<i64>,
    error_message: Option<&str>,
    saved_query_id: Option<i64>,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO query_history 
        (connection_name, submitted_by, sql, status, execution_time_ms, row_count, error_message, saved_query_id)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(connection_name)
    .bind(submitted_by.as_str())
    .bind(sql)
    .bind(status.as_str())
    .bind(execution_time_ms)
    .bind(row_count)
    .bind(error_message)
    .bind(saved_query_id)
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to record query: {e}")))?;

    let id = result.last_insert_rowid();

    prune_old_entries(pool).await?;

    Ok(id)
}

/// Prunes history entries beyond retention limits.
async fn prune_old_entries(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM query_history
        WHERE created_at < datetime('now', ? || ' days')
        "#,
    )
    .bind(-MAX_HISTORY_DAYS)
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to prune old entries: {e}")))?;

    sqlx::query(
        r#"
        DELETE FROM query_history
        WHERE id NOT IN (
            SELECT id FROM query_history
            ORDER BY created_at DESC
            LIMIT ?
        )
        "#,
    )
    .bind(MAX_HISTORY_ENTRIES)
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to prune excess entries: {e}")))?;

    Ok(())
}

/// Lists history entries with optional filters.
pub async fn list_history(pool: &SqlitePool, filter: &HistoryFilter) -> Result<Vec<HistoryEntry>> {
    let mut query = String::from(
        r#"
        SELECT id, connection_name, submitted_by, sql, status, 
               execution_time_ms, row_count, error_message, saved_query_id, created_at
        FROM query_history
        WHERE 1=1
        "#,
    );

    if filter.connection_name.is_some() {
        query.push_str(" AND connection_name = ?");
    }
    if filter.text_search.is_some() {
        query.push_str(" AND sql LIKE ?");
    }
    if filter.since_days.is_some() {
        query.push_str(" AND created_at >= datetime('now', ? || ' days')");
    }

    query.push_str(" ORDER BY created_at DESC");

    if filter.limit.is_some() {
        query.push_str(" LIMIT ?");
    }

    let mut sqlx_query = sqlx::query_as::<_, HistoryEntryRow>(&query);

    if let Some(ref conn) = filter.connection_name {
        sqlx_query = sqlx_query.bind(conn);
    }
    if let Some(ref text) = filter.text_search {
        sqlx_query = sqlx_query.bind(format!("%{}%", text));
    }
    if let Some(days) = filter.since_days {
        sqlx_query = sqlx_query.bind(-days);
    }
    if let Some(limit) = filter.limit {
        sqlx_query = sqlx_query.bind(limit);
    }

    let rows = sqlx_query
        .fetch_all(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to list history: {e}")))?;

    Ok(rows.into_iter().map(HistoryEntry::from).collect())
}

/// Gets a single history entry by ID.
pub async fn get_history_entry(pool: &SqlitePool, id: i64) -> Result<Option<HistoryEntry>> {
    let row: Option<HistoryEntryRow> = sqlx::query_as(
        r#"
        SELECT id, connection_name, submitted_by, sql, status,
               execution_time_ms, row_count, error_message, saved_query_id, created_at
        FROM query_history
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to get history entry: {e}")))?;

    Ok(row.map(HistoryEntry::from))
}

/// Clears all history entries.
pub async fn clear_history(pool: &SqlitePool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM query_history")
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to clear history: {e}")))?;

    Ok(result.rows_affected())
}

/// Clears history for a specific connection.
pub async fn clear_connection_history(pool: &SqlitePool, connection_name: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM query_history WHERE connection_name = ?")
        .bind(connection_name)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to clear history: {e}")))?;

    Ok(result.rows_affected())
}

/// Returns the count of history entries.
pub async fn count_history(pool: &SqlitePool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM query_history")
        .fetch_one(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to count history: {e}")))?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        migrations::run_migrations(&pool).await.unwrap();

        sqlx::query("INSERT INTO connections (name, database) VALUES ('test', 'testdb')")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_record_and_list_history() {
        let pool = test_pool().await;

        let id = record_query(
            &pool,
            "test",
            SubmittedBy::User,
            "SELECT 1",
            QueryStatus::Success,
            Some(10),
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        assert!(id > 0);

        let entries = list_history(&pool, &HistoryFilter::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sql, "SELECT 1");
        assert_eq!(entries[0].submitted_by, SubmittedBy::User);
        assert_eq!(entries[0].status, QueryStatus::Success);
    }

    #[tokio::test]
    async fn test_filter_by_connection() {
        let pool = test_pool().await;

        sqlx::query("INSERT INTO connections (name, database) VALUES ('other', 'otherdb')")
            .execute(&pool)
            .await
            .unwrap();

        record_query(&pool, "test", SubmittedBy::User, "SELECT 1", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();
        record_query(&pool, "other", SubmittedBy::User, "SELECT 2", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();

        let filter = HistoryFilter {
            connection_name: Some("test".to_string()),
            ..Default::default()
        };

        let entries = list_history(&pool, &filter).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sql, "SELECT 1");
    }

    #[tokio::test]
    async fn test_filter_by_text() {
        let pool = test_pool().await;

        record_query(&pool, "test", SubmittedBy::User, "SELECT * FROM users", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();
        record_query(&pool, "test", SubmittedBy::User, "SELECT * FROM orders", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();

        let filter = HistoryFilter {
            text_search: Some("users".to_string()),
            ..Default::default()
        };

        let entries = list_history(&pool, &filter).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].sql.contains("users"));
    }

    #[tokio::test]
    async fn test_clear_history() {
        let pool = test_pool().await;

        record_query(&pool, "test", SubmittedBy::User, "SELECT 1", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();
        record_query(&pool, "test", SubmittedBy::User, "SELECT 2", QueryStatus::Success, None, None, None, None)
            .await
            .unwrap();

        let deleted = clear_history(&pool).await.unwrap();
        assert_eq!(deleted, 2);

        let count = count_history(&pool).await.unwrap();
        assert_eq!(count, 0);
    }
}
