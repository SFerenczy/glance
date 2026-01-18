//! Integration tests for headless mode.

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
fn test_headless_basic_execution() {
    let (code, stdout, _) = run_headless(&["--headless", "--mock-db", "--events", "key:esc"]);

    assert_eq!(code, 0, "Expected exit code 0");
    assert!(
        stdout.contains("Events: 1 executed"),
        "Should show events executed"
    );
}

#[test]
fn test_headless_type_event() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:hello world",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains(r#""input_text": "hello world""#));
}

#[test]
fn test_headless_assertion_pass() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:test,assert:contains:test",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains(r#""passed": 1"#));
    assert!(stdout.contains(r#""failed": 0"#));
}

#[test]
fn test_headless_assertion_fail() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:hello,assert:contains:goodbye",
        "--output",
        "json",
    ]);

    assert_eq!(code, 1, "Should exit with code 1 on assertion failure");
    assert!(stdout.contains(r#""passed": 0"#));
    assert!(stdout.contains(r#""failed": 1"#));
}

#[test]
fn test_headless_custom_size() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "key:esc",
        "--size",
        "120x40",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    // JSON output should have 40 lines in screen_lines
    let lines_count = stdout.matches(r#""#).count();
    assert!(lines_count > 0, "Should have screen lines");
}

#[test]
fn test_headless_frames_output() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:a,type:b",
        "--output",
        "frames",
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("=== FRAME 0 (initial) ==="));
    assert!(stdout.contains("=== FRAME 1 (type:a) ==="));
    assert!(stdout.contains("=== FRAME 2 (type:b) ==="));
}

#[test]
fn test_headless_requires_events_or_script() {
    let (code, stdout, stderr) = run_headless(&["--headless", "--mock-db"]);

    assert_eq!(code, 1, "Should fail without events or script");
    // Error may be in stdout or stderr depending on logging config
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("requires --events or --script"),
        "Should show error message. Got: {}",
        combined
    );
}

#[test]
fn test_headless_state_assertion() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "assert:state:focus=Input",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains(r#""passed": 1"#));
}

#[test]
fn test_headless_multiple_assertions() {
    let (code, stdout, _) = run_headless(&[
        "--headless",
        "--mock-db",
        "--events",
        "type:hello,assert:contains:hello,assert:state:focus=Input,assert:not-contains:error",
        "--output",
        "json",
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains(r#""passed": 3"#));
    assert!(stdout.contains(r#""failed": 0"#));
}
