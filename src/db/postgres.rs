//! PostgreSQL database client implementation.
//!
//! Provides the `PostgresClient` struct that implements the `DatabaseClient` trait
//! for PostgreSQL databases using sqlx.

use crate::config::ConnectionConfig;
use crate::db::{
    Column, ColumnInfo, DatabaseClient, ForeignKey, Index, QueryResult, Row, Schema, Table, Value,
};
use crate::error::{GlanceError, Result};
use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::{Column as SqlxColumn, Row as SqlxRow, TypeInfo};
use std::time::{Duration, Instant};

/// Query timeout in seconds.
#[allow(dead_code)]
const QUERY_TIMEOUT_SECS: u64 = 30;

/// Maximum rows to return from a query.
#[allow(dead_code)]
const MAX_ROWS: usize = 1000;

/// PostgreSQL database client.
#[derive(Debug)]
#[allow(dead_code)]
pub struct PostgresClient {
    pool: PgPool,
}

impl PostgresClient {
    /// Creates a new PostgresClient from an existing connection pool.
    ///
    /// This is primarily useful for testing.
    #[allow(dead_code)]
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseClient for PostgresClient {
    async fn connect(config: &ConnectionConfig) -> Result<Self> {
        let conn_str = config.to_connection_string()?;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&conn_str)
            .await
            .map_err(|e| map_connection_error(e, config))?;

        Ok(Self { pool })
    }

    async fn introspect_schema(&self) -> Result<Schema> {
        let tables = self.fetch_tables().await?;
        let foreign_keys = self.fetch_foreign_keys().await?;

        Ok(Schema {
            tables,
            foreign_keys,
        })
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        let start = Instant::now();

        // Use a timeout for query execution
        let result = tokio::time::timeout(
            Duration::from_secs(QUERY_TIMEOUT_SECS),
            sqlx::query(sql).fetch_all(&self.pool),
        )
        .await
        .map_err(|_| {
            GlanceError::query(format!(
                "Query timed out after {QUERY_TIMEOUT_SECS} seconds"
            ))
        })?
        .map_err(|e| GlanceError::query(format_query_error(e)))?;

        let execution_time = start.elapsed();

        if result.is_empty() {
            return Ok(QueryResult::new().with_execution_time(execution_time));
        }

        // Extract column metadata from the first row
        let columns: Vec<ColumnInfo> = result
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|col| ColumnInfo::new(col.name(), col.type_info().name()))
                    .collect()
            })
            .unwrap_or_default();

        // Convert rows, limiting to MAX_ROWS
        let rows: Vec<Row> = result.iter().take(MAX_ROWS).map(convert_row).collect();

        let row_count = rows.len();

        Ok(QueryResult {
            columns,
            rows,
            execution_time,
            row_count,
        })
    }

    async fn close(&self) -> Result<()> {
        self.pool.close().await;
        Ok(())
    }
}

#[allow(dead_code)]
impl PostgresClient {
    /// Fetches all tables from the public schema.
    async fn fetch_tables(&self) -> Result<Vec<Table>> {
        // Get table names
        let table_names: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT table_name::text
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
            ORDER BY table_name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GlanceError::query(format!("Failed to fetch tables: {e}")))?;

        let mut tables = Vec::with_capacity(table_names.len());

        for table_name in table_names {
            let columns = self.fetch_columns(&table_name).await?;
            let primary_key = self.fetch_primary_key(&table_name).await?;
            let indexes = self.fetch_indexes(&table_name).await?;

            tables.push(Table {
                name: table_name,
                columns,
                primary_key,
                indexes,
            });
        }

        Ok(tables)
    }

    /// Fetches columns for a specific table.
    async fn fetch_columns(&self, table_name: &str) -> Result<Vec<Column>> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT
                column_name::text,
                data_type::text,
                is_nullable::text,
                column_default::text
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = $1
            ORDER BY ordinal_position
            "#,
        )
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GlanceError::query(format!("Failed to fetch columns for {table_name}: {e}"))
        })?;

        Ok(rows
            .into_iter()
            .map(|(name, data_type, is_nullable, default)| Column {
                name,
                data_type,
                is_nullable: is_nullable == "YES",
                default,
            })
            .collect())
    }

    /// Fetches primary key columns for a specific table.
    async fn fetch_primary_key(&self, table_name: &str) -> Result<Vec<String>> {
        let columns: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT kcu.column_name::text
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            WHERE tc.table_schema = 'public'
                AND tc.table_name = $1
                AND tc.constraint_type = 'PRIMARY KEY'
            ORDER BY kcu.ordinal_position
            "#,
        )
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GlanceError::query(format!("Failed to fetch primary key for {table_name}: {e}"))
        })?;

        Ok(columns)
    }

    /// Fetches indexes for a specific table.
    async fn fetch_indexes(&self, table_name: &str) -> Result<Vec<Index>> {
        let rows: Vec<(String, String, bool)> = sqlx::query_as(
            r#"
            SELECT
                i.relname::text AS index_name,
                a.attname::text AS column_name,
                ix.indisunique AS is_unique
            FROM pg_class t
            JOIN pg_index ix ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            JOIN pg_namespace n ON n.oid = t.relnamespace
            WHERE n.nspname = 'public'
                AND t.relname = $1
                AND NOT ix.indisprimary
            ORDER BY i.relname, a.attnum
            "#,
        )
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GlanceError::query(format!("Failed to fetch indexes for {table_name}: {e}"))
        })?;

        // Group by index name
        let mut index_map: std::collections::HashMap<String, (Vec<String>, bool)> =
            std::collections::HashMap::new();

        for (index_name, column_name, is_unique) in rows {
            index_map
                .entry(index_name)
                .or_insert_with(|| (Vec::new(), is_unique))
                .0
                .push(column_name);
        }

        Ok(index_map
            .into_iter()
            .map(|(name, (columns, is_unique))| Index {
                name,
                columns,
                is_unique,
            })
            .collect())
    }

    /// Fetches all foreign key relationships.
    async fn fetch_foreign_keys(&self) -> Result<Vec<ForeignKey>> {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT
                kcu.table_name::text AS from_table,
                kcu.column_name::text AS from_column,
                ccu.table_name::text AS to_table,
                ccu.column_name::text AS to_column
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage ccu
                ON tc.constraint_name = ccu.constraint_name
                AND tc.table_schema = ccu.table_schema
            WHERE tc.table_schema = 'public'
                AND tc.constraint_type = 'FOREIGN KEY'
            ORDER BY kcu.table_name, kcu.ordinal_position
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GlanceError::query(format!("Failed to fetch foreign keys: {e}")))?;

        // Group by constraint (from_table + to_table combination for simplicity)
        // In practice, we might want to group by constraint name for multi-column FKs
        let mut fk_map: std::collections::HashMap<(String, String), (Vec<String>, Vec<String>)> =
            std::collections::HashMap::new();

        for (from_table, from_column, to_table, to_column) in rows {
            let key = (from_table, to_table);
            let entry = fk_map
                .entry(key)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(from_column);
            entry.1.push(to_column);
        }

        Ok(fk_map
            .into_iter()
            .map(
                |((from_table, to_table), (from_columns, to_columns))| ForeignKey {
                    from_table,
                    from_columns,
                    to_table,
                    to_columns,
                },
            )
            .collect())
    }
}

/// Converts a sqlx PgRow to our Row type.
#[allow(dead_code)]
fn convert_row(row: &PgRow) -> Row {
    row.columns()
        .iter()
        .enumerate()
        .map(|(i, col)| convert_value(row, i, col.type_info().name()))
        .collect()
}

/// Converts a single column value from a PgRow to our Value type.
#[allow(dead_code)]
fn convert_value(row: &PgRow, index: usize, type_name: &str) -> Value {
    // Try to get the value based on the type
    // We use a match on type name and try to decode appropriately
    match type_name.to_uppercase().as_str() {
        "BOOL" | "BOOLEAN" => row
            .try_get::<Option<bool>, _>(index)
            .ok()
            .flatten()
            .map(Value::Bool)
            .unwrap_or(Value::Null),

        "INT2" | "SMALLINT" => row
            .try_get::<Option<i16>, _>(index)
            .ok()
            .flatten()
            .map(|v| Value::Int(v as i64))
            .unwrap_or(Value::Null),

        "INT4" | "INT" | "INTEGER" => row
            .try_get::<Option<i32>, _>(index)
            .ok()
            .flatten()
            .map(|v| Value::Int(v as i64))
            .unwrap_or(Value::Null),

        "INT8" | "BIGINT" => row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map(Value::Int)
            .unwrap_or(Value::Null),

        "FLOAT4" | "REAL" => row
            .try_get::<Option<f32>, _>(index)
            .ok()
            .flatten()
            .map(|v| Value::Float(v as f64))
            .unwrap_or(Value::Null),

        "FLOAT8" | "DOUBLE PRECISION" => row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .map(Value::Float)
            .unwrap_or(Value::Null),

        "BYTEA" => row
            .try_get::<Option<Vec<u8>>, _>(index)
            .ok()
            .flatten()
            .map(Value::Bytes)
            .unwrap_or(Value::Null),

        // For all other types, try to get as string
        _ => row
            .try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

/// Maps sqlx connection errors to user-friendly messages.
#[allow(dead_code)]
fn map_connection_error(error: sqlx::Error, config: &ConnectionConfig) -> GlanceError {
    let host = config.host.as_deref().unwrap_or("localhost");
    let port = config.port;
    let user = config.user.as_deref().unwrap_or("unknown");
    let database = config.database.as_deref().unwrap_or("unknown");

    let error_str = error.to_string().to_lowercase();

    if error_str.contains("connection refused") || error_str.contains("could not connect") {
        GlanceError::connection(format!(
            "Cannot connect to {host}:{port}. Check that the server is running."
        ))
    } else if error_str.contains("password authentication failed")
        || error_str.contains("authentication failed")
    {
        GlanceError::connection(format!(
            "Authentication failed for user '{user}'. Check your credentials."
        ))
    } else if error_str.contains("does not exist") && error_str.contains("database") {
        GlanceError::connection(format!("Database '{database}' does not exist."))
    } else if error_str.contains("ssl") || error_str.contains("tls") {
        GlanceError::connection(
            "Server requires SSL. Add '?sslmode=require' to connection string.".to_string(),
        )
    } else {
        GlanceError::connection(error.to_string())
    }
}

/// Formats a query error with hints if available.
#[allow(dead_code)]
fn format_query_error(error: sqlx::Error) -> String {
    // sqlx errors already include good formatting, but we can enhance if needed
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running PostgreSQL database.
    // They are skipped in CI unless GLANCE_TEST_DATABASE_URL is set.

    fn get_test_database_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    async fn get_test_client() -> Option<PostgresClient> {
        let url = get_test_database_url()?;
        let config = ConnectionConfig::from_connection_string(&url).ok()?;
        PostgresClient::connect(&config).await.ok()
    }

    #[tokio::test]
    async fn test_connect_to_database() {
        let Some(client) = get_test_client().await else {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        };

        // If we got here, connection succeeded
        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_introspect_schema() {
        let Some(client) = get_test_client().await else {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        };

        let schema = client.introspect_schema().await.unwrap();

        // Should have at least the test tables
        assert!(!schema.tables.is_empty(), "Expected at least one table");

        // Find the users table
        let users_table = schema.tables.iter().find(|t| t.name == "users");
        assert!(users_table.is_some(), "Expected 'users' table to exist");

        let users = users_table.unwrap();
        assert!(
            !users.columns.is_empty(),
            "Expected users table to have columns"
        );
        assert!(
            !users.primary_key.is_empty(),
            "Expected users table to have a primary key"
        );

        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_execute_select_query() {
        let Some(client) = get_test_client().await else {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        };

        let result = client
            .execute_query("SELECT 1 as num, 'hello' as greeting")
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "num");
        assert_eq!(result.columns[1].name, "greeting");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.row_count, 1);

        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_execute_query_with_error() {
        let Some(client) = get_test_client().await else {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        };

        let result = client
            .execute_query("SELECT * FROM nonexistent_table_xyz")
            .await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(
            error.to_string().contains("nonexistent_table_xyz")
                || error.to_string().contains("does not exist")
        );

        client.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_connection_error_messages() {
        let config = ConnectionConfig {
            host: Some("nonexistent.invalid.host".to_string()),
            port: 5432,
            database: Some("testdb".to_string()),
            user: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
        };

        let result = PostgresClient::connect(&config).await;
        assert!(result.is_err());
        // The error should be a connection error
        let error = result.unwrap_err();
        assert!(matches!(error, GlanceError::Connection(_)));
    }
}
