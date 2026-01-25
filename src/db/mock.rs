//! Mock database client for testing.
//!
//! Provides an in-memory database implementation for headless testing.

use super::{ColumnInfo, DatabaseClient, QueryResult, Schema, Value};
use crate::error::{GlanceError, Result};
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
    async fn introspect_schema(&self) -> Result<Schema> {
        Ok(self.schema.clone())
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        // Parse simple SELECT queries and return mock results
        let sql_upper = sql.to_uppercase();

        if sql_upper.starts_with("SELECT") {
            // Special case: queries with "WHERE 1 = 0" return empty results with column metadata
            if sql_upper.contains("WHERE 1 = 0") || sql_upper.contains("WHERE 1=0") {
                // Extract column names from the SELECT clause for better testing
                let columns = if sql_upper.contains("SELECT ID, EMAIL") || sql_upper.contains("SELECT ID,EMAIL") {
                    vec![
                        ColumnInfo {
                            name: "id".to_string(),
                            data_type: "integer".to_string(),
                        },
                        ColumnInfo {
                            name: "email".to_string(),
                            data_type: "text".to_string(),
                        },
                    ]
                } else {
                    vec![
                        ColumnInfo {
                            name: "result".to_string(),
                            data_type: "text".to_string(),
                        }
                    ]
                };

                Ok(QueryResult {
                    columns,
                    rows: vec![],
                    execution_time: Duration::from_millis(1),
                    row_count: 0,
                    total_rows: Some(0),
                    was_truncated: false,
                })
            } else {
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
            }
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

/// A database client that always fails queries with an error.
///
/// Useful for testing error handling paths.
#[derive(Debug)]
pub struct FailingDatabaseClient {
    schema: Schema,
    error_message: String,
}

impl FailingDatabaseClient {
    /// Creates a new failing database client with a default error message.
    pub fn new() -> Self {
        Self {
            schema: Schema::default(),
            error_message: "Mock database error".to_string(),
        }
    }

    /// Creates a new failing database client with a custom error message.
    #[allow(dead_code)]
    pub fn with_error(error_message: String) -> Self {
        Self {
            schema: Schema::default(),
            error_message,
        }
    }

    /// Creates a new failing database client with a custom schema.
    #[allow(dead_code)]
    pub fn with_schema(schema: Schema) -> Self {
        Self {
            schema,
            error_message: "Mock database error".to_string(),
        }
    }
}

impl Default for FailingDatabaseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseClient for FailingDatabaseClient {
    async fn introspect_schema(&self) -> Result<Schema> {
        Ok(self.schema.clone())
    }

    async fn execute_query(&self, _sql: &str) -> Result<QueryResult> {
        Err(GlanceError::query(self.error_message.clone()))
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

    #[tokio::test]
    async fn test_mock_empty_result_with_columns() {
        let client = MockDatabaseClient::new();
        let result = client
            .execute_query("SELECT id, email FROM users WHERE 1 = 0")
            .await
            .unwrap();

        // Should return empty rows
        assert_eq!(result.row_count, 0);
        assert_eq!(result.rows.len(), 0);
        assert!(result.is_empty());

        // But should have column metadata
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[0].data_type, "integer");
        assert_eq!(result.columns[1].name, "email");
        assert_eq!(result.columns[1].data_type, "text");
    }

    #[tokio::test]
    async fn test_failing_client_returns_error() {
        let client = FailingDatabaseClient::new();
        let result = client.execute_query("SELECT 1").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.category(), "Query Error");
        assert!(error.to_string().contains("Mock database error"));
    }

    #[tokio::test]
    async fn test_failing_client_with_custom_error() {
        let client = FailingDatabaseClient::with_error("Custom error message".to_string());
        let result = client.execute_query("SELECT 1").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Custom error message"));
    }

    #[tokio::test]
    async fn test_failing_client_introspect_succeeds() {
        let client = FailingDatabaseClient::new();
        let schema = client.introspect_schema().await.unwrap();
        assert_eq!(schema.tables.len(), 0); // Default schema is empty
    }
}
