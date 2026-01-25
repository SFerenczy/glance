//! Saved queries and tags persistence.
//!
//! CRUD operations for user-curated SQL queries with tagging support.

#![allow(dead_code)]

use crate::error::{GlanceError, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;

/// A saved query with its tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub sql: String,
    pub description: Option<String>,
    pub connection_name: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub usage_count: i64,
}

/// Raw database row for saved query (without tags).
#[derive(Debug, Clone, FromRow)]
struct SavedQueryRow {
    id: i64,
    name: String,
    sql: String,
    description: Option<String>,
    connection_name: Option<String>,
    created_at: String,
    updated_at: String,
    last_used_at: Option<String>,
    usage_count: i64,
}

/// A tag associated with a saved query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQueryTag {
    pub saved_query_id: i64,
    pub tag: String,
}

/// Filter options for querying saved queries.
#[derive(Debug, Clone, Default)]
pub struct SavedQueryFilter {
    pub connection_name: Option<String>,
    pub include_global: bool,
    /// Multiple tags to filter by (AND semantics - query must have all tags).
    pub tags: Option<Vec<String>>,
    pub text_search: Option<String>,
    pub limit: Option<i64>,
}

/// Creates a new saved query.
pub async fn create_saved_query(
    pool: &SqlitePool,
    name: &str,
    sql: &str,
    description: Option<&str>,
    connection_name: Option<&str>,
    tags: &[String],
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO saved_queries (name, sql, description, connection_name)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(name)
    .bind(sql)
    .bind(description)
    .bind(connection_name)
    .execute(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint") {
            GlanceError::persistence(format!(
                "Saved query '{}' already exists for this connection",
                name
            ))
        } else {
            GlanceError::persistence(format!("Failed to create saved query: {e}"))
        }
    })?;

    let id = result.last_insert_rowid();

    for tag in tags {
        add_tag(pool, id, tag).await?;
    }

    Ok(id)
}

/// Adds a tag to a saved query.
async fn add_tag(pool: &SqlitePool, saved_query_id: i64, tag: &str) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO saved_query_tags (saved_query_id, tag) VALUES (?, ?)")
        .bind(saved_query_id)
        .bind(tag)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to add tag: {e}")))?;

    Ok(())
}

/// Gets tags for a saved query.
async fn get_tags(pool: &SqlitePool, saved_query_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT tag FROM saved_query_tags WHERE saved_query_id = ? ORDER BY tag")
            .bind(saved_query_id)
            .fetch_all(pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to get tags: {e}")))?;

    Ok(rows.into_iter().map(|(t,)| t).collect())
}

/// Gets a saved query by ID.
pub async fn get_saved_query(pool: &SqlitePool, id: i64) -> Result<Option<SavedQuery>> {
    let row: Option<SavedQueryRow> = sqlx::query_as(
        r#"
        SELECT id, name, sql, description, connection_name, 
               created_at, updated_at, last_used_at, usage_count
        FROM saved_queries
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to get saved query: {e}")))?;

    match row {
        Some(r) => {
            let tags = get_tags(pool, r.id).await?;
            Ok(Some(SavedQuery {
                id: r.id,
                name: r.name,
                sql: r.sql,
                description: r.description,
                connection_name: r.connection_name,
                tags,
                created_at: r.created_at,
                updated_at: r.updated_at,
                last_used_at: r.last_used_at,
                usage_count: r.usage_count,
            }))
        }
        None => Ok(None),
    }
}

/// Gets a saved query by name and optional connection.
pub async fn get_saved_query_by_name(
    pool: &SqlitePool,
    name: &str,
    connection_name: Option<&str>,
) -> Result<Option<SavedQuery>> {
    let row: Option<SavedQueryRow> = if let Some(conn) = connection_name {
        sqlx::query_as(
            r#"
            SELECT id, name, sql, description, connection_name,
                   created_at, updated_at, last_used_at, usage_count
            FROM saved_queries
            WHERE name = ? AND (connection_name = ? OR connection_name IS NULL)
            ORDER BY CASE WHEN connection_name = ? THEN 0 ELSE 1 END
            LIMIT 1
            "#,
        )
        .bind(name)
        .bind(conn)
        .bind(conn)
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT id, name, sql, description, connection_name,
                   created_at, updated_at, last_used_at, usage_count
            FROM saved_queries
            WHERE name = ? AND connection_name IS NULL
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await
    }
    .map_err(|e| GlanceError::persistence(format!("Failed to get saved query: {e}")))?;

    match row {
        Some(r) => {
            let tags = get_tags(pool, r.id).await?;
            Ok(Some(SavedQuery {
                id: r.id,
                name: r.name,
                sql: r.sql,
                description: r.description,
                connection_name: r.connection_name,
                tags,
                created_at: r.created_at,
                updated_at: r.updated_at,
                last_used_at: r.last_used_at,
                usage_count: r.usage_count,
            }))
        }
        None => Ok(None),
    }
}

/// Lists saved queries with optional filters.
pub async fn list_saved_queries(
    pool: &SqlitePool,
    filter: &SavedQueryFilter,
) -> Result<Vec<SavedQuery>> {
    let mut conditions = vec!["1=1".to_string()];
    let mut bindings: Vec<String> = vec![];

    if let Some(ref conn) = filter.connection_name {
        if filter.include_global {
            conditions.push("(connection_name = ? OR connection_name IS NULL)".to_string());
        } else {
            conditions.push("connection_name = ?".to_string());
        }
        bindings.push(conn.clone());
    }

    // Multi-tag filtering with AND semantics: query must have all specified tags
    if let Some(ref tags) = filter.tags {
        if !tags.is_empty() {
            // For each tag, add a condition that the query has that tag
            for tag in tags {
                conditions.push(
                    "id IN (SELECT saved_query_id FROM saved_query_tags WHERE tag = ?)".to_string(),
                );
                bindings.push(tag.clone());
            }
        }
    }

    if let Some(ref text) = filter.text_search {
        conditions.push("(name LIKE ? OR sql LIKE ? OR description LIKE ?)".to_string());
        let pattern = format!("%{}%", text);
        bindings.push(pattern.clone());
        bindings.push(pattern.clone());
        bindings.push(pattern);
    }

    let query = format!(
        r#"
        SELECT id, name, sql, description, connection_name,
               created_at, updated_at, last_used_at, usage_count
        FROM saved_queries
        WHERE {}
        ORDER BY name
        {}
        "#,
        conditions.join(" AND "),
        filter
            .limit
            .map(|l| format!("LIMIT {}", l))
            .unwrap_or_default()
    );

    let mut sqlx_query = sqlx::query_as::<_, SavedQueryRow>(&query);
    for binding in &bindings {
        sqlx_query = sqlx_query.bind(binding);
    }

    let rows = sqlx_query
        .fetch_all(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to list saved queries: {e}")))?;

    let mut queries = Vec::with_capacity(rows.len());
    for row in rows {
        let tags = get_tags(pool, row.id).await?;
        queries.push(SavedQuery {
            id: row.id,
            name: row.name,
            sql: row.sql,
            description: row.description,
            connection_name: row.connection_name,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_used_at: row.last_used_at,
            usage_count: row.usage_count,
        });
    }

    Ok(queries)
}

/// Updates a saved query.
pub async fn update_saved_query(
    pool: &SqlitePool,
    id: i64,
    sql: Option<&str>,
    description: Option<&str>,
    tags: Option<&[String]>,
) -> Result<()> {
    if let Some(new_sql) = sql {
        sqlx::query("UPDATE saved_queries SET sql = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(new_sql)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to update saved query: {e}")))?;
    }

    if let Some(new_desc) = description {
        sqlx::query(
            "UPDATE saved_queries SET description = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(new_desc)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update saved query: {e}")))?;
    }

    if let Some(new_tags) = tags {
        sqlx::query("DELETE FROM saved_query_tags WHERE saved_query_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to update tags: {e}")))?;

        for tag in new_tags {
            add_tag(pool, id, tag).await?;
        }
    }

    Ok(())
}

/// Deletes a saved query.
pub async fn delete_saved_query(pool: &SqlitePool, id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM saved_queries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to delete saved query: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(GlanceError::persistence("Saved query not found"));
    }

    Ok(())
}

/// Deletes a saved query by name.
pub async fn delete_saved_query_by_name(
    pool: &SqlitePool,
    name: &str,
    connection_name: Option<&str>,
) -> Result<()> {
    let result = if let Some(conn) = connection_name {
        sqlx::query("DELETE FROM saved_queries WHERE name = ? AND connection_name = ?")
            .bind(name)
            .bind(conn)
            .execute(pool)
            .await
    } else {
        sqlx::query("DELETE FROM saved_queries WHERE name = ? AND connection_name IS NULL")
            .bind(name)
            .execute(pool)
            .await
    }
    .map_err(|e| GlanceError::persistence(format!("Failed to delete saved query: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(GlanceError::persistence(format!(
            "Saved query '{}' not found",
            name
        )));
    }

    Ok(())
}

/// Records usage of a saved query.
pub async fn record_usage(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE saved_queries 
        SET usage_count = usage_count + 1, 
            last_used_at = datetime('now'),
            updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to record usage: {e}")))?;

    Ok(())
}

/// Checks if a tag is global (prefixed with "global:").
pub fn is_global_tag(tag: &str) -> bool {
    tag.starts_with("global:")
}

/// Strips the "global:" prefix from a tag if present.
pub fn normalize_tag(tag: &str) -> &str {
    tag.strip_prefix("global:").unwrap_or(tag)
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
    async fn test_create_and_get_saved_query() {
        let pool = test_pool().await;

        let id = create_saved_query(
            &pool,
            "my_query",
            "SELECT * FROM users",
            Some("Get all users"),
            Some("test"),
            &["users".to_string(), "select".to_string()],
        )
        .await
        .unwrap();

        let query = get_saved_query(&pool, id).await.unwrap().unwrap();
        assert_eq!(query.name, "my_query");
        assert_eq!(query.sql, "SELECT * FROM users");
        assert_eq!(query.description, Some("Get all users".to_string()));
        assert_eq!(query.tags, vec!["select", "users"]);
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let pool = test_pool().await;

        create_saved_query(&pool, "my_query", "SELECT 1", None, Some("test"), &[])
            .await
            .unwrap();

        let query = get_saved_query_by_name(&pool, "my_query", Some("test"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(query.name, "my_query");
    }

    #[tokio::test]
    async fn test_list_with_tag_filter() {
        let pool = test_pool().await;

        create_saved_query(
            &pool,
            "q1",
            "SELECT 1",
            None,
            Some("test"),
            &["tag1".to_string()],
        )
        .await
        .unwrap();
        create_saved_query(
            &pool,
            "q2",
            "SELECT 2",
            None,
            Some("test"),
            &["tag2".to_string()],
        )
        .await
        .unwrap();

        let filter = SavedQueryFilter {
            tags: Some(vec!["tag1".to_string()]),
            ..Default::default()
        };

        let queries = list_saved_queries(&pool, &filter).await.unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "q1");
    }

    #[tokio::test]
    async fn test_list_with_multiple_tags_filter() {
        let pool = test_pool().await;

        // Query with both tag1 and tag2
        create_saved_query(
            &pool,
            "q1",
            "SELECT 1",
            None,
            Some("test"),
            &["tag1".to_string(), "tag2".to_string()],
        )
        .await
        .unwrap();

        // Query with only tag1
        create_saved_query(
            &pool,
            "q2",
            "SELECT 2",
            None,
            Some("test"),
            &["tag1".to_string()],
        )
        .await
        .unwrap();

        // Filter by both tags (AND semantics) - should only return q1
        let filter = SavedQueryFilter {
            tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
            ..Default::default()
        };

        let queries = list_saved_queries(&pool, &filter).await.unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "q1");
    }

    #[tokio::test]
    async fn test_delete_saved_query() {
        let pool = test_pool().await;

        let id = create_saved_query(&pool, "to_delete", "SELECT 1", None, Some("test"), &[])
            .await
            .unwrap();

        delete_saved_query(&pool, id).await.unwrap();

        let query = get_saved_query(&pool, id).await.unwrap();
        assert!(query.is_none());
    }

    #[tokio::test]
    async fn test_record_usage() {
        let pool = test_pool().await;

        let id = create_saved_query(&pool, "used_query", "SELECT 1", None, Some("test"), &[])
            .await
            .unwrap();

        record_usage(&pool, id).await.unwrap();
        record_usage(&pool, id).await.unwrap();

        let query = get_saved_query(&pool, id).await.unwrap().unwrap();
        assert_eq!(query.usage_count, 2);
        assert!(query.last_used_at.is_some());
    }

    #[test]
    fn test_is_global_tag() {
        assert!(is_global_tag("global:common"));
        assert!(!is_global_tag("common"));
    }

    #[test]
    fn test_normalize_tag() {
        assert_eq!(normalize_tag("global:common"), "common");
        assert_eq!(normalize_tag("common"), "common");
    }
}
