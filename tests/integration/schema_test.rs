//! Schema introspection integration tests.
//!
//! Tests database schema discovery functionality.

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

#[tokio::test]
async fn test_introspect_tables() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();

    // Should have at least the test tables (users, orders)
    assert!(
        schema.tables.len() >= 2,
        "Expected at least 2 tables, got {}",
        schema.tables.len()
    );

    // Verify users table exists
    let users_table = schema.tables.iter().find(|t| t.name == "users");
    assert!(users_table.is_some(), "Expected 'users' table to exist");

    // Verify orders table exists
    let orders_table = schema.tables.iter().find(|t| t.name == "orders");
    assert!(orders_table.is_some(), "Expected 'orders' table to exist");

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_introspect_columns() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();

    let users_table = schema
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("users table should exist");

    // Check expected columns exist
    let column_names: Vec<&str> = users_table
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(column_names.contains(&"id"), "Expected 'id' column");
    assert!(column_names.contains(&"email"), "Expected 'email' column");
    assert!(column_names.contains(&"name"), "Expected 'name' column");
    assert!(
        column_names.contains(&"created_at"),
        "Expected 'created_at' column"
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_introspect_primary_key() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();

    let users_table = schema
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("users table should exist");

    // Users table should have 'id' as primary key
    assert!(
        users_table.primary_key.contains(&"id".to_string()),
        "Expected 'id' to be primary key, got: {:?}",
        users_table.primary_key
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_introspect_foreign_keys() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();

    // Should have foreign key from orders.user_id to users.id
    let fk = schema
        .foreign_keys
        .iter()
        .find(|fk| fk.from_table == "orders" && fk.to_table == "users");

    assert!(
        fk.is_some(),
        "Expected foreign key from orders to users, got: {:?}",
        schema.foreign_keys
    );

    let fk = fk.unwrap();
    assert!(
        fk.from_columns.contains(&"user_id".to_string()),
        "Expected from_columns to contain 'user_id'"
    );
    assert!(
        fk.to_columns.contains(&"id".to_string()),
        "Expected to_columns to contain 'id'"
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_introspect_indexes() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();

    let orders_table = schema
        .tables
        .iter()
        .find(|t| t.name == "orders")
        .expect("orders table should exist");

    // Should have indexes on orders table
    assert!(
        !orders_table.indexes.is_empty(),
        "Expected orders table to have indexes"
    );

    // Check for user_id index
    let user_id_index = orders_table
        .indexes
        .iter()
        .find(|idx| idx.columns.contains(&"user_id".to_string()));
    assert!(
        user_id_index.is_some(),
        "Expected index on user_id column, got: {:?}",
        orders_table.indexes
    );

    client.close().await.unwrap();
}

#[tokio::test]
async fn test_schema_format_for_llm() {
    let Some(client) = get_test_client().await else {
        eprintln!("Skipping test: DATABASE_URL not set");
        return;
    };

    let schema = client.introspect_schema().await.unwrap();
    let formatted = schema.format_for_llm();

    // Check that the formatted output contains expected elements
    assert!(
        formatted.contains("Table: users"),
        "Expected 'Table: users' in formatted output"
    );
    assert!(
        formatted.contains("Table: orders"),
        "Expected 'Table: orders' in formatted output"
    );
    assert!(
        formatted.contains("Foreign Keys:"),
        "Expected 'Foreign Keys:' section"
    );
    assert!(
        formatted.contains("orders.user_id -> users.id"),
        "Expected foreign key reference in output"
    );

    client.close().await.unwrap();
}
