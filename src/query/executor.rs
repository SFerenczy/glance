//! Query execution with safety classification.
//!
//! Provides isolated query execution that can be tested independently
//! of the full orchestrator.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::db::{DatabaseClient, QueryResult};
use crate::error::{GlanceError, Result};
use crate::persistence::{self, OwnedRecordQueryParams, QueryStatus, StateDb, SubmittedBy};
use crate::safety::{classify_sql, ClassificationResult, SafetyLevel};
use crate::tui::app::{QueryLogEntry, QuerySource};

/// Query executor that handles SQL classification and execution.
#[allow(dead_code)]
pub struct QueryExecutor<'a> {
    db: &'a dyn DatabaseClient,
    state_db: Option<&'a Arc<StateDb>>,
    connection_name: Option<&'a str>,
}

#[allow(dead_code)]
impl<'a> QueryExecutor<'a> {
    /// Creates a new query executor.
    pub fn new(
        db: &'a dyn DatabaseClient,
        state_db: Option<&'a Arc<StateDb>>,
        connection_name: Option<&'a str>,
    ) -> Self {
        Self {
            db,
            state_db,
            connection_name,
        }
    }

    /// Classify and potentially execute a query.
    ///
    /// Safe queries are executed immediately. Mutating/destructive queries
    /// return `NeedsConfirmation` for user approval.
    pub async fn execute(&self, sql: &str, source: QuerySource) -> ExecutionResult {
        let classification = classify_sql(sql);

        match classification.level {
            SafetyLevel::Safe => {
                let effective_source = if source == QuerySource::Manual {
                    QuerySource::Manual
                } else {
                    QuerySource::Auto
                };
                match self.execute_immediate(sql, effective_source).await {
                    Ok(outcome) => ExecutionResult::Success(outcome),
                    Err(e) => ExecutionResult::Error(e),
                }
            }
            SafetyLevel::Mutating | SafetyLevel::Destructive => {
                ExecutionResult::NeedsConfirmation {
                    sql: sql.to_string(),
                    classification,
                }
            }
        }
    }

    /// Execute without classification (for confirmed queries).
    pub async fn execute_confirmed(&self, sql: &str, source: QuerySource) -> Result<QueryOutcome> {
        self.execute_immediate(sql, source).await
    }

    /// Execute a query immediately without classification.
    async fn execute_immediate(&self, sql: &str, source: QuerySource) -> Result<QueryOutcome> {
        let start = Instant::now();
        let result = self.db.execute_query(sql).await;
        let execution_time = start.elapsed();

        let (status, row_count, error_msg) = match &result {
            Ok(qr) => (QueryStatus::Success, Some(qr.row_count as i64), None),
            Err(e) => (QueryStatus::Error, None, Some(e.to_string())),
        };

        if let Some(state_db) = self.state_db {
            let pool = state_db.pool().clone();
            let params = OwnedRecordQueryParams {
                connection_name: self.connection_name.unwrap_or("default").to_string(),
                submitted_by: SubmittedBy::User,
                sql: sql.to_string(),
                status,
                execution_time_ms: Some(execution_time.as_millis() as i64),
                row_count,
                error_message: error_msg,
                saved_query_id: None,
            };
            tokio::spawn(async move {
                let _ = persistence::history::record_query_owned(&pool, params).await;
            });
        }

        let log_entry = match &result {
            Ok(query_result) => QueryLogEntry::success_with_source(
                sql.to_string(),
                execution_time,
                query_result.row_count,
                source,
            ),
            Err(e) => QueryLogEntry::error_with_source(
                sql.to_string(),
                execution_time,
                e.to_string(),
                source,
            ),
        };

        match result {
            Ok(query_result) => Ok(QueryOutcome {
                result: query_result,
                execution_time,
                log_entry,
            }),
            Err(e) => Err(GlanceError::query(e.to_string())),
        }
    }
}

/// Result of executing a query.
#[derive(Debug)]
#[allow(dead_code)]
pub enum ExecutionResult {
    /// Query executed successfully.
    Success(QueryOutcome),
    /// Query needs user confirmation before execution.
    NeedsConfirmation {
        sql: String,
        classification: ClassificationResult,
    },
    /// Query execution failed.
    Error(GlanceError),
}

/// Successful query execution outcome.
#[derive(Debug)]
#[allow(dead_code)]
pub struct QueryOutcome {
    /// The query result.
    pub result: QueryResult,
    /// How long the query took to execute.
    pub execution_time: Duration,
    /// Log entry for the query.
    pub log_entry: QueryLogEntry,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MockDatabaseClient;

    #[tokio::test]
    async fn test_execute_safe_query() {
        let mock_db = MockDatabaseClient::new();
        let executor = QueryExecutor::new(&mock_db, None, None);

        let result = executor
            .execute("SELECT * FROM users", QuerySource::Manual)
            .await;

        match result {
            ExecutionResult::Success(outcome) => {
                assert!(outcome.result.row_count > 0);
                assert_eq!(outcome.log_entry.source, QuerySource::Manual);
            }
            _ => panic!("Expected Success result"),
        }
    }

    #[tokio::test]
    async fn test_execute_mutating_query_needs_confirmation() {
        let mock_db = MockDatabaseClient::new();
        let executor = QueryExecutor::new(&mock_db, None, None);

        let result = executor
            .execute(
                "INSERT INTO users (name) VALUES ('test')",
                QuerySource::Generated,
            )
            .await;

        match result {
            ExecutionResult::NeedsConfirmation {
                sql,
                classification,
            } => {
                assert!(sql.contains("INSERT"));
                assert_eq!(classification.level, SafetyLevel::Mutating);
            }
            _ => panic!("Expected NeedsConfirmation result"),
        }
    }

    #[tokio::test]
    async fn test_execute_destructive_query_needs_confirmation() {
        let mock_db = MockDatabaseClient::new();
        let executor = QueryExecutor::new(&mock_db, None, None);

        let result = executor
            .execute("DELETE FROM users", QuerySource::Generated)
            .await;

        match result {
            ExecutionResult::NeedsConfirmation {
                sql,
                classification,
            } => {
                assert!(sql.contains("DELETE"));
                assert_eq!(classification.level, SafetyLevel::Destructive);
            }
            _ => panic!("Expected NeedsConfirmation result"),
        }
    }

    #[tokio::test]
    async fn test_execute_confirmed_bypasses_classification() {
        let mock_db = MockDatabaseClient::new();
        let executor = QueryExecutor::new(&mock_db, None, None);

        let result = executor
            .execute_confirmed(
                "INSERT INTO users (name) VALUES ('test')",
                QuerySource::Generated,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_auto_source_for_generated_safe_queries() {
        let mock_db = MockDatabaseClient::new();
        let executor = QueryExecutor::new(&mock_db, None, None);

        let result = executor
            .execute("SELECT * FROM users", QuerySource::Generated)
            .await;

        match result {
            ExecutionResult::Success(outcome) => {
                assert_eq!(outcome.log_entry.source, QuerySource::Auto);
            }
            _ => panic!("Expected Success result"),
        }
    }
}
