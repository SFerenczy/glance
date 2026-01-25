//! Integration tests for the persistence layer.

use db_glance::db::DatabaseBackend;
use db_glance::persistence::{
    self, ConnectionProfile, HistoryFilter, OwnedRecordQueryParams, QueryStatus, SavedQueryFilter,
    StateDb, SubmittedBy,
};
use std::time::{Duration, Instant};
use tempfile::tempdir;

async fn create_test_db() -> (StateDb, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_state.db");
    let db = StateDb::open(&path).await.unwrap();
    (db, dir)
}

#[tokio::test]
async fn test_state_db_creation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.db");

    let db = StateDb::open(&path).await.unwrap();
    assert!(path.exists());
    db.close().await;
}

#[tokio::test]
async fn test_connection_crud() {
    let (db, _dir) = create_test_db().await;

    let profile = ConnectionProfile {
        name: "test_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "testdb".to_string(),
        host: Some("localhost".to_string()),
        port: 5432,
        username: Some("testuser".to_string()),
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };

    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let retrieved = persistence::connections::get_connection(db.pool(), "test_conn")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.name, "test_conn");
    assert_eq!(retrieved.database, "testdb");

    let connections = persistence::connections::list_connections(db.pool())
        .await
        .unwrap();
    assert_eq!(connections.len(), 1);

    persistence::connections::delete_connection(db.pool(), "test_conn", db.secrets())
        .await
        .unwrap();

    let deleted = persistence::connections::get_connection(db.pool(), "test_conn")
        .await
        .unwrap();
    assert!(deleted.is_none());

    db.close().await;
}

#[tokio::test]
async fn test_query_history() {
    let (db, _dir) = create_test_db().await;

    let profile = ConnectionProfile {
        name: "hist_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "histdb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };
    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let id = persistence::history::record_query(
        db.pool(),
        "hist_conn",
        SubmittedBy::User,
        "SELECT * FROM users",
        QueryStatus::Success,
        Some(50),
        Some(10),
        None,
        None,
    )
    .await
    .unwrap();

    assert!(id > 0);

    let entries = persistence::history::list_history(db.pool(), &HistoryFilter::default())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].sql, "SELECT * FROM users");
    assert_eq!(entries[0].status, QueryStatus::Success);

    let filter = HistoryFilter {
        text_search: Some("users".to_string()),
        ..Default::default()
    };
    let filtered = persistence::history::list_history(db.pool(), &filter)
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);

    let count = persistence::history::clear_history(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);

    db.close().await;
}

#[tokio::test]
async fn test_saved_queries_with_tags() {
    let (db, _dir) = create_test_db().await;

    let profile = ConnectionProfile {
        name: "sq_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "sqdb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };
    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let id = persistence::saved_queries::create_saved_query(
        db.pool(),
        "get_users",
        "SELECT * FROM users",
        Some("Fetch all users"),
        Some("sq_conn"),
        &["users".to_string(), "select".to_string()],
    )
    .await
    .unwrap();

    let query = persistence::saved_queries::get_saved_query(db.pool(), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(query.name, "get_users");
    assert_eq!(query.sql, "SELECT * FROM users");
    assert_eq!(query.tags.len(), 2);
    assert!(query.tags.contains(&"users".to_string()));

    let filter = SavedQueryFilter {
        tags: Some(vec!["users".to_string()]),
        ..Default::default()
    };
    let by_tag = persistence::saved_queries::list_saved_queries(db.pool(), &filter)
        .await
        .unwrap();
    assert_eq!(by_tag.len(), 1);

    persistence::saved_queries::record_usage(db.pool(), id)
        .await
        .unwrap();
    let updated = persistence::saved_queries::get_saved_query(db.pool(), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.usage_count, 1);

    persistence::saved_queries::delete_saved_query(db.pool(), id)
        .await
        .unwrap();

    let deleted = persistence::saved_queries::get_saved_query(db.pool(), id)
        .await
        .unwrap();
    assert!(deleted.is_none());

    db.close().await;
}

#[tokio::test]
async fn test_llm_settings() {
    let (db, _dir) = create_test_db().await;

    let settings = persistence::llm_settings::get_llm_settings(db.pool())
        .await
        .unwrap();
    assert_eq!(settings.provider, "openai");
    assert_eq!(settings.model, "gpt-5");

    persistence::llm_settings::set_provider(db.pool(), "anthropic")
        .await
        .unwrap();

    let updated = persistence::llm_settings::get_llm_settings(db.pool())
        .await
        .unwrap();
    assert_eq!(updated.provider, "anthropic");

    persistence::llm_settings::set_model(db.pool(), "claude-3-5-sonnet-latest")
        .await
        .unwrap();

    let final_settings = persistence::llm_settings::get_llm_settings(db.pool())
        .await
        .unwrap();
    assert_eq!(final_settings.model, "claude-3-5-sonnet-latest");

    db.close().await;
}

#[tokio::test]
async fn test_invalid_provider_rejected() {
    let (db, _dir) = create_test_db().await;

    let result = persistence::llm_settings::set_provider(db.pool(), "invalid").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid provider"));

    db.close().await;
}

#[tokio::test]
async fn test_duplicate_connection_rejected() {
    let (db, _dir) = create_test_db().await;

    let profile = ConnectionProfile {
        name: "dup_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "dupdb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };

    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let result =
        persistence::connections::create_connection(db.pool(), &profile, None, db.secrets()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));

    db.close().await;
}

#[tokio::test]
async fn test_concurrent_read_access() {
    let db = StateDb::open_in_memory().await.unwrap();

    let profile = ConnectionProfile {
        name: "concurrent_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "concurrentdb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };
    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    for i in 0..5 {
        persistence::history::record_query(
            db.pool(),
            "concurrent_conn",
            SubmittedBy::User,
            &format!("SELECT {}", i),
            QueryStatus::Success,
            Some(10),
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();
    }

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let pool = db.pool().clone();
            tokio::spawn(async move {
                persistence::history::list_history(&pool, &HistoryFilter::default()).await
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 5);
    }

    db.close().await;
}

#[tokio::test]
async fn test_write_doesnt_block_reads_excessively() {
    let db = StateDb::open_in_memory().await.unwrap();

    let profile = ConnectionProfile {
        name: "blocking_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "blockingdb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };
    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let write_pool = db.pool().clone();
    let write_handle = tokio::spawn(async move {
        persistence::history::record_query(
            &write_pool,
            "blocking_conn",
            SubmittedBy::User,
            "SELECT 1",
            QueryStatus::Success,
            Some(10),
            Some(1),
            None,
            None,
        )
        .await
    });

    let start = Instant::now();
    let _ = persistence::history::list_history(db.pool(), &HistoryFilter::default()).await;
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "Read took too long: {:?}",
        start.elapsed()
    );

    write_handle.await.unwrap().unwrap();
    db.close().await;
}

#[tokio::test]
async fn test_record_query_owned() {
    let db = StateDb::open_in_memory().await.unwrap();

    let profile = ConnectionProfile {
        name: "owned_conn".to_string(),
        backend: DatabaseBackend::default(),
        database: "owneddb".to_string(),
        host: None,
        port: 5432,
        username: None,
        sslmode: None,
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };
    persistence::connections::create_connection(db.pool(), &profile, None, db.secrets())
        .await
        .unwrap();

    let params = OwnedRecordQueryParams {
        connection_name: "owned_conn".to_string(),
        submitted_by: SubmittedBy::User,
        sql: "SELECT * FROM test".to_string(),
        status: QueryStatus::Success,
        execution_time_ms: Some(42),
        row_count: Some(10),
        error_message: None,
        saved_query_id: None,
    };

    let id = persistence::history::record_query_owned(db.pool(), params)
        .await
        .unwrap();
    assert!(id > 0);

    let entries = persistence::history::list_history(db.pool(), &HistoryFilter::default())
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].sql, "SELECT * FROM test");
    assert_eq!(entries[0].execution_time_ms, Some(42));

    db.close().await;
}
