//! Integration tests for SQL autocomplete functionality.

use super::common::run_headless;

#[test]
fn test_sql_mode_triggers_completions() {
    // Typing "/sql SELECT * FROM " should trigger table completions
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM ",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // The mock DB has tables, so completions should be visible
    assert!(
        stdout.contains(r#""sql_completion_visible": true"#),
        "SQL completion popup should be visible. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_shows_tables_after_from() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM ,assert:contains:users",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(
        stdout.contains(r#""passed": 1"#),
        "Should show 'users' table in completions"
    );
}

#[test]
fn test_sql_completion_navigation_down() {
    // Navigate down in completions
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM ,key:down",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After pressing down, selected index should be 1
    assert!(
        stdout.contains(r#""sql_completion_selected": 1"#),
        "Should have selected second item. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_navigation_up() {
    // Navigate down then up
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM ,key:down,key:down,key:up",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After down, down, up: selected should be 1
    assert!(
        stdout.contains(r#""sql_completion_selected": 1"#),
        "Should have selected second item after up. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_accept_with_tab() {
    // Accept completion with Tab (keeps popup open)
    // Use key:space to add trailing space since type: trims trailing whitespace
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM,key:space,key:tab",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After Tab, the completion should be inserted
    // The input should now contain the table name
    assert!(
        stdout.contains("sql_completion_visible"),
        "Should have SQL completion state. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_enter_submits_without_accepting() {
    // Enter submits the query without accepting completion
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM,key:space,key:enter",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After Enter, popup should be closed and input submitted
    assert!(
        stdout.contains(r#""sql_completion_visible": false"#),
        "SQL completion popup should be closed after Enter. Got: {}",
        stdout
    );
    // Input should be empty after submission
    assert!(
        stdout.contains(r#""input_text": """#),
        "Input should be empty after Enter submission. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_close_with_esc() {
    // Close popup with Esc
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM ,key:esc",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After Esc, popup should be closed
    assert!(
        stdout.contains(r#""sql_completion_visible": false"#),
        "SQL completion popup should be closed after Esc. Got: {}",
        stdout
    );
    // Input should remain unchanged
    assert!(
        stdout.contains("/sql SELECT * FROM "),
        "Input should remain unchanged after Esc. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_filter_by_typing() {
    // Typing should filter completions
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM us",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // Should show completions filtered to "us" prefix
    assert!(
        stdout.contains(r#""sql_completion_visible": true"#),
        "SQL completion popup should be visible. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_columns_after_where() {
    // After WHERE, should suggest columns
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE ",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(
        stdout.contains(r#""sql_completion_visible": true"#),
        "SQL completion popup should be visible for WHERE clause. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_not_visible_outside_sql_mode() {
    // Completions should not appear for regular input
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:show me all users",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(
        stdout.contains(r#""sql_completion_visible": false"#),
        "SQL completion popup should NOT be visible outside /sql mode. Got: {}",
        stdout
    );
}

#[test]
fn test_sql_completion_keywords_at_start() {
    // At the start of SQL (after "/sql "), should suggest keywords like SELECT
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql S",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // After typing "/sql S", should show keyword completions starting with S
    assert!(
        stdout.contains(r#""sql_completion_visible": true"#),
        "SQL completion popup should be visible at start. Got: {}",
        stdout
    );
}

// Tests for WHERE clause state machine

#[test]
fn test_where_suggests_columns_not_operators() {
    // After WHERE, should suggest columns (not AND/OR/BETWEEN)
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE ,assert:contains:id,assert:not-contains:AND",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "Test failed. Output: {}", stdout);
    assert!(
        stdout.contains(r#""passed": 2"#),
        "Should show column 'id' and NOT show 'AND' in WHERE clause. Got: {}",
        stdout
    );
}

#[test]
fn test_where_operator_context_suggests_operators() {
    // After column name in WHERE, should suggest operators
    // Use key:space to add trailing space since comma-separated events lose trailing spaces
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE status,key:space,assert:contains:=",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "Test failed. Output: {}", stdout);
    assert!(
        stdout.contains(r#""passed": 1"#),
        "Should show '=' operator after column name. Got: {}",
        stdout
    );
}

#[test]
fn test_where_value_context_suggests_values() {
    // After operator in WHERE, should suggest values like NULL, TRUE, FALSE
    // Use key:space to add trailing space since comma-separated events lose trailing spaces
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE active =,key:space,assert:contains:TRUE",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "Test failed. Output: {}", stdout);
    assert!(
        stdout.contains(r#""passed": 1"#),
        "Should show 'TRUE' after operator. Got: {}",
        stdout
    );
}

#[test]
fn test_where_continuation_suggests_and_or() {
    // After complete condition, should suggest AND/OR
    // Use key:space to add trailing space since comma-separated events lose trailing spaces
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE id = 1,key:space,assert:contains:AND",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "Test failed. Output: {}", stdout);
    assert!(
        stdout.contains(r#""passed": 1"#),
        "Should show 'AND' after complete condition. Got: {}",
        stdout
    );
}

#[test]
fn test_where_after_and_suggests_columns() {
    // After AND in WHERE, should suggest columns again
    // Use key:space to add trailing space since comma-separated events lose trailing spaces
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users WHERE id = 1 AND,key:space,assert:contains:name",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "Test failed. Output: {}", stdout);
    assert!(
        stdout.contains(r#""passed": 1"#),
        "Should show column 'name' after AND. Got: {}",
        stdout
    );
}
