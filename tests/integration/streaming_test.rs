//! Streaming query execution integration tests.
//!
//! Tests that query execution uses bounded memory via streaming fetch.

use db_glance::config::ConnectionConfig;
use db_glance::db::{DatabaseClient, PostgresClient};

/// Helper to get test database URL from environment.
fn get_test_database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

/// Helper to create a test client.
async fn get_test_client() -> Option<PostgresClient> {
    let url = get_test_database_url()?;
    let config = ConnectionConfig::from_connection_string(&url).ok()?;
    PostgresClient::connect(&config).await.ok()
}

/// Scenario: Query with fewer rows than MAX_ROWS
/// Given a query that returns a small number of rows
/// When execute_query is called
/// Then all rows are returned
/// And was_truncated is false
/// And total_rows equals row_count
#[tokio::test]
async fn test_streaming_small_result_set() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT * FROM users LIMIT 10")
        .await
        .unwrap();

    assert!(result.row_count <= 10);
    assert!(!result.was_truncated);
    assert_eq!(result.total_rows, Some(result.row_count));

    client.close().await.unwrap();
}

/// Scenario: Query with no rows
/// Given a query that returns 0 rows
/// When execute_query is called
/// Then 0 rows are returned
/// And column metadata is still available
#[tokio::test]
async fn test_streaming_empty_result_set() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT id, email, name FROM users WHERE 1 = 0")
        .await
        .unwrap();

    assert_eq!(result.row_count, 0);
    assert!(result.rows.is_empty());
    assert!(!result.was_truncated);
    assert_eq!(result.total_rows, Some(0));
    // Column metadata should still be available via fallback
    assert!(
        !result.columns.is_empty(),
        "Expected column metadata for empty result"
    );
    assert_eq!(result.columns.len(), 3);

    client.close().await.unwrap();
}

/// Scenario: Query with more rows than MAX_ROWS
/// Given a query that returns more than MAX_ROWS (1000)
/// When execute_query is called
/// Then exactly MAX_ROWS rows are returned
/// And was_truncated is true
/// And total_rows is None (unknown, since we stopped early)
#[tokio::test]
async fn test_streaming_truncated_result_set() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    // Generate more than MAX_ROWS (1000) using generate_series
    let result = client
        .execute_query("SELECT generate_series(1, 2000) as n")
        .await
        .unwrap();

    // Should have exactly MAX_ROWS (1000) rows
    assert_eq!(result.row_count, 1000);
    assert_eq!(result.rows.len(), 1000);
    assert!(result.was_truncated);
    // total_rows should be None because we stopped streaming early
    assert_eq!(result.total_rows, None);

    client.close().await.unwrap();
}

/// Scenario: Verify truncation warning message
/// Given a truncated result
/// When truncation_warning() is called
/// Then it returns a warning message
#[tokio::test]
async fn test_truncation_warning_message() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT generate_series(1, 2000) as n")
        .await
        .unwrap();

    let warning = result.truncation_warning();
    assert!(warning.is_some());
    let warning_msg = warning.unwrap();
    assert!(warning_msg.contains("truncated"));
    assert!(warning_msg.contains("1000"));

    client.close().await.unwrap();
}

/// Scenario: Non-truncated result has no warning
/// Given a non-truncated result
/// When truncation_warning() is called
/// Then it returns None
#[tokio::test]
async fn test_no_truncation_warning_for_small_result() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT * FROM users LIMIT 5")
        .await
        .unwrap();

    assert!(result.truncation_warning().is_none());

    client.close().await.unwrap();
}

/// Scenario: Column metadata extracted from first row
/// Given a query that returns rows
/// When execute_query is called
/// Then column metadata is correctly extracted
#[tokio::test]
async fn test_streaming_column_metadata() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT 1 as int_col, 'text' as text_col, true as bool_col")
        .await
        .unwrap();

    assert_eq!(result.columns.len(), 3);
    assert_eq!(result.columns[0].name, "int_col");
    assert_eq!(result.columns[1].name, "text_col");
    assert_eq!(result.columns[2].name, "bool_col");

    client.close().await.unwrap();
}

/// Scenario: Streaming handles errors gracefully
/// Given a query that fails mid-execution
/// When execute_query is called
/// Then an appropriate error is returned
#[tokio::test]
async fn test_streaming_handles_query_error() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT * FROM nonexistent_table_for_streaming_test")
        .await;

    assert!(result.is_err());

    client.close().await.unwrap();
}
