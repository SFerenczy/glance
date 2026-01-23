//! Common test utilities for TUI tests.

use std::path::PathBuf;
use std::process::Command;

/// Returns the path to the glance binary.
/// The binary is already built by `cargo test` as part of the workspace.
/// Uses CARGO_BIN_EXE_glance which is set by cargo test to the correct path.
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_glance"))
}

/// Run glance in headless mode with the given arguments.
/// Uses a pre-built binary instead of `cargo run` for speed.
pub fn run_headless(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(binary_path())
        .args(args)
        .output()
        .expect("Failed to execute command");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}
