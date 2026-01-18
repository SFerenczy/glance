//! Integration tests for LLM settings commands.
//!
//! Tests /llm provider, /llm model, and /llm key commands.

use super::common::run_headless;

#[test]
fn test_llm_settings_view() {
    // Scenario: View current LLM settings
    // Given a connected database
    // When I type "/llm" and press Enter
    // Then I should see the current LLM settings
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm,key:enter,wait:100ms,assert:contains:LLM settings,assert:contains:Provider",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_provider_change() {
    // Scenario: Change LLM provider
    // Given a connected database
    // When I type "/llm provider anthropic" and press Enter
    // Then I should see confirmation that the provider was changed
    // And the conversation should be cleared
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm provider anthropic,key:enter,wait:100ms,assert:contains:LLM provider set to 'anthropic',assert:contains:Conversation cleared",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_model_change() {
    // Scenario: Change LLM model
    // Given a connected database
    // When I type "/llm model claude-3-opus" and press Enter
    // Then I should see confirmation that the model was changed
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm model claude-3-opus,key:enter,wait:100ms,assert:contains:LLM model set to 'claude-3-opus'",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_key_status_not_configured() {
    // Scenario: View API key status when not configured
    // Given a connected database with no API key set
    // When I type "/llm key" and press Enter
    // Then I should see that the API key is not configured
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm key,key:enter,wait:100ms,assert:contains:API key status,assert:contains:Not configured",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_key_set_masked() {
    // Scenario: Set API key with masked output
    // Given a connected database
    // When I type "/llm key sk-test-1234567890abcdef" and press Enter
    // Then I should see confirmation with masked key (****...cdef)
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm key sk-test-1234567890abcdef,key:enter,wait:100ms,assert:contains:API key set for provider,assert:contains:****...cdef",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_key_persisted() {
    // Scenario: API key is persisted after setting
    // Given a connected database
    // When I set an API key and then check the status
    // Then I should see that the key is configured
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm key test-key-12345678,key:enter,wait:100ms,type:/llm key,key:enter,wait:100ms,assert:not-contains:Not configured",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_invalid_provider() {
    // Scenario: Invalid provider name
    // Given a connected database
    // When I type "/llm provider invalid" and press Enter
    // Then I should see an error message
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:/llm provider invalid,key:enter,wait:100ms,assert:contains:Invalid provider",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0, "All assertions should pass. stdout: {}", stdout);
}

#[test]
fn test_llm_settings_flow_script() {
    // Run the full LLM settings flow from script file
    let (code, stdout, stderr) = run_headless(&[
        "--headless",
        "--mock-db",
        "--script",
        "tests/tui/fixtures/llm_settings_flow.txt",
        "--output",
        "json",
    ]);

    if code != 0 {
        eprintln!("Script failed. stdout:\n{}\nstderr:\n{}", stdout, stderr);
    }
    assert_eq!(code, 0, "LLM settings flow script should pass");
}
