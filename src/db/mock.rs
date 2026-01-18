//! Mock database client for testing.
//!
//! Provides an in-memory database implementation for headless testing.

use super::{ColumnInfo, DatabaseClient, QueryResult, Schema, Value};
use crate::config::ConnectionConfig;
use crate::error::Result;
use async_trait::async_trait;
use std::time::Duration;

/// A mock database client that returns predefined results.
pub struct MockDatabaseClient {
    schema: Schema,
}

impl MockDatabaseClient {
    /// Creates a new mock database client with an empty schema.
    pub fn new() -> Self {
        Self {
            schema: Schema::default(),
        }
    }

    /// Creates a new mock database client with the given schema.
    #[allow(dead_code)]
    pub fn with_schema(schema: Schema) -> Self {
        Self { schema }
    }
}

impl Default for MockDatabaseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseClient for MockDatabaseClient {
    async fn connect(_config: &ConnectionConfig) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self::new())
    }

    async fn introspect_schema(&self) -> Result<Schema> {
        Ok(self.schema.clone())
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        // Parse simple SELECT queries and return mock results
        let sql_upper = sql.to_uppercase();

        if sql_upper.starts_with("SELECT") {
            // Return a simple result with one row
            let columns = vec![ColumnInfo {
                name: "result".to_string(),
                data_type: "text".to_string(),
            }];

            let rows = vec![vec![Value::String(format!("Mock result for: {}", sql))]];

            Ok(QueryResult {
                columns,
                rows,
                execution_time: Duration::from_millis(1),
                row_count: 1,
                total_rows: Some(1),
                was_truncated: false,
            })
        } else {
            // For non-SELECT queries, return empty result
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                execution_time: Duration::from_millis(1),
                row_count: 0,
                total_rows: Some(0),
                was_truncated: false,
            })
        }
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_select() {
        let client = MockDatabaseClient::new();
        let result = client.execute_query("SELECT 1").await.unwrap();
        assert_eq!(result.row_count, 1);
        assert_eq!(result.columns.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_insert() {
        let client = MockDatabaseClient::new();
        let result = client
            .execute_query("INSERT INTO test VALUES (1)")
            .await
            .unwrap();
        assert_eq!(result.row_count, 0);
    }
}
