//! Integration tests for saved queries commands.
//!
//! Tests /savequery, /queries, /usequery, and /query delete commands.

use std::process::Command;

fn run_headless(args: &[&str]) -> (i32, String, String) {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to execute command");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

#[test]
fn test_queries_list_empty() {
    // Scenario: List saved queries when none exist
    // Given a connected database with no saved queries
    // When I type "/queries" and press Enter
    // Then I should see "No saved queries found"
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/queries,key:enter,wait:100ms,assert:contains:No saved queries found",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_savequery_and_list() {
    // Scenario: Save a query and list it
    // Given a connected database
    // When I execute a SQL query and save it with tags
    // Then I should see confirmation
    // And the query should appear in the list
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users,key:enter,wait:100ms,type:/savequery get_users #users #select,key:enter,wait:100ms,assert:contains:Query saved as 'get_users',type:/queries,key:enter,wait:100ms,assert:contains:get_users,assert:contains:#users",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_usequery_loads_sql() {
    // Scenario: Use a saved query loads it into input
    // Given a saved query exists
    // When I type "/usequery <name>" and press Enter
    // Then I should see the query loaded with /sql prefix
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT 1,key:enter,wait:100ms,type:/savequery test_query,key:enter,wait:100ms,type:/usequery test_query,key:enter,wait:100ms,assert:contains:Loaded query 'test_query',assert:contains:/sql SELECT 1",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_query_delete() {
    // Scenario: Delete a saved query
    // Given a saved query exists
    // When I type "/query delete <name>" and press Enter
    // Then I should see confirmation of deletion
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT 1,key:enter,wait:100ms,type:/savequery to_delete,key:enter,wait:100ms,type:/query delete to_delete,key:enter,wait:100ms,assert:contains:Saved query 'to_delete' deleted",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_queries_filter_by_tag() {
    // Scenario: Filter queries by tag
    // Given multiple saved queries with different tags
    // When I type "/queries --tag <tag>" and press Enter
    // Then I should only see queries with that tag in the results
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT 1,key:enter,wait:100ms,type:/savequery q1 #tag1,key:enter,wait:100ms,type:/sql SELECT 2,key:enter,wait:100ms,type:/savequery q2 #tag2,key:enter,wait:100ms,type:/queries --tag tag1,key:enter,wait:100ms,assert:contains:q1 (test) [#tag1]",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_queries_text_search() {
    // Scenario: Search queries by text
    // Given multiple saved queries
    // When I type "/queries --text <search>" and press Enter
    // Then I should see matching queries in the results
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/sql SELECT * FROM users,key:enter,wait:100ms,type:/savequery user_query,key:enter,wait:100ms,type:/sql SELECT * FROM orders,key:enter,wait:100ms,type:/savequery order_query,key:enter,wait:100ms,type:/queries --text user,key:enter,wait:100ms,assert:contains:user_query (test)",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_savequery_usage_error() {
    // Scenario: Save query without name shows usage
    // Given a connected database
    // When I type "/savequery" without a name and press Enter
    // Then I should see usage instructions
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/savequery,key:enter,wait:100ms,assert:contains:Usage",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_usequery_not_found() {
    // Scenario: Use non-existent query shows error
    // Given no saved queries
    // When I type "/usequery nonexistent" and press Enter
    // Then I should see an error message
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/usequery nonexistent,key:enter,wait:100ms,assert:contains:not found",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_saved_queries_flow_script() {
    // Run the full saved queries flow from script file
    let (code, stdout, stderr) = run_headless(&[
        "--headless",
        "--mock-db",
        "--script",
        "tests/tui/fixtures/saved_queries_flow.txt",
        "--output",
        "json",
    ]);

    if code != 0 {
        eprintln!("Script failed. stdout:\n{}\nstderr:\n{}", stdout, stderr);
    }
    assert_eq!(code, 0, "Saved queries flow script should pass");
}
