//! Query execution integration tests.
//!
//! Tests SQL query execution and result handling.

use db_glance::config::ConnectionConfig;
use db_glance::db::{DatabaseClient, PostgresClient, Value};

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

#[tokio::test]
async fn test_execute_simple_select() {
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
async fn test_execute_select_from_users() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT id, email, name FROM users ORDER BY id LIMIT 5")
        .await
        .unwrap();

    assert_eq!(result.columns.len(), 3);
    assert!(result.row_count <= 5);
    assert!(!result.was_truncated);

    // Verify first row has expected structure
    if !result.rows.is_empty() {
        let first_row = &result.rows[0];
        assert_eq!(first_row.len(), 3);

        // First column should be an integer (id)
        match &first_row[0] {
            Value::Int(_) => {}
            other => panic!("Expected Int for id, got {:?}", other),
        }

        // Second column should be a string (email)
        match &first_row[1] {
            Value::String(_) => {}
            other => panic!("Expected String for email, got {:?}", other),
        }
    }

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_select_with_null() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    // Carol has NULL name in test data
    let result = client
        .execute_query("SELECT name FROM users WHERE email = 'carol@example.com'")
        .await
        .unwrap();

    if !result.rows.is_empty() {
        let name_value = &result.rows[0][0];
        assert!(name_value.is_null(), "Expected NULL for Carol's name");
    }

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_select_with_join() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query(
            "SELECT u.email, o.total 
             FROM users u 
             JOIN orders o ON u.id = o.user_id 
             ORDER BY o.id 
             LIMIT 5",
        )
        .await
        .unwrap();

    assert_eq!(result.columns.len(), 2);
    assert!(result.row_count <= 5);

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_select_with_aggregation() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT COUNT(*) as count FROM users")
        .await
        .unwrap();

    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.columns[0].name, "count");
    assert_eq!(result.row_count, 1);

    // Count should be a positive integer
    if let Value::Int(count) = &result.rows[0][0] {
        assert!(*count > 0, "Expected positive count");
    }

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_query_with_syntax_error() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client.execute_query("SELEC * FROM users").await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("syntax") || error_msg.contains("error"),
        "Expected syntax error, got: {}",
        error_msg
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_query_with_nonexistent_table() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT * FROM nonexistent_table_xyz_123")
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("does not exist") || error_msg.contains("not exist"),
        "Expected 'does not exist' error, got: {}",
        error_msg
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_query_with_nonexistent_column() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT nonexistent_column FROM users")
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("does not exist") || error_msg.contains("column"),
        "Expected column error, got: {}",
        error_msg
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execute_empty_result() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client
        .execute_query("SELECT * FROM users WHERE 1 = 0")
        .await
        .unwrap();

    assert!(result.is_empty());
    assert_eq!(result.row_count, 0);

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_execution_time_recorded() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let result = client.execute_query("SELECT * FROM users").await.unwrap();

    // Execution time should be recorded and positive
    assert!(
        !result.execution_time.is_zero(),
        "Expected non-zero execution time"
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_value_display_strings() {
    // Test Value display formatting
    assert_eq!(Value::Null.to_display_string(), "NULL");
    assert_eq!(Value::Bool(true).to_display_string(), "true");
    assert_eq!(Value::Bool(false).to_display_string(), "false");
    assert_eq!(Value::Int(42).to_display_string(), "42");
    assert_eq!(Value::Int(-100).to_display_string(), "-100");
    assert_eq!(Value::Float(3.14).to_display_string(), "3.14");
    assert_eq!(
        Value::String("hello".to_string()).to_display_string(),
        "hello"
    );
    assert_eq!(Value::Bytes(vec![1, 2, 3]).to_display_string(), "<3 bytes>");
}
