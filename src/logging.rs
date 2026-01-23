//! Logging configuration for Glance.
//!
//! Provides platform-aware logging initialization that writes to files in TUI mode
//! (to avoid corrupting the terminal display) and stderr in headless mode.

use std::fs::{self, File};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// Initializes logging for TUI mode.
///
/// Logs are written to a file to avoid corrupting the terminal display.
/// Location: `~/.local/state/glance/glance.log` on Linux (XDG state directory),
/// or the platform-appropriate state/config directory on other systems.
pub fn init_file_logging() {
    let log_path = get_log_path();

    // Ensure parent directory exists
    if let Some(parent) = log_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Warning: Could not create log directory: {e}");
            // Fall back to no logging rather than corrupting TUI
            return;
        }
    }

    // Open log file (truncate on each run to avoid unbounded growth)
    let log_file = match File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Could not create log file: {e}");
            return;
        }
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(log_file)
        .with_ansi(false) // No ANSI colors in file output
        .init();
}

/// Initializes logging for headless mode.
///
/// Logs are written to stderr for easy debugging and test output capture.
pub fn init_stderr_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

/// Returns the path for the log file.
///
/// Uses XDG state directory on Linux (`~/.local/state/glance/glance.log`),
/// or falls back to config directory on other platforms.
pub fn get_log_path() -> PathBuf {
    // Try state directory first (XDG_STATE_HOME on Linux)
    if let Some(state_dir) = dirs::state_dir() {
        return state_dir.join("glance").join("glance.log");
    }

    // Fall back to config directory
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("glance").join("glance.log");
    }

    // Last resort: temp directory
    std::env::temp_dir().join("glance.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_path_is_absolute() {
        let path = get_log_path();
        assert!(path.is_absolute());
    }

    #[test]
    fn test_log_path_ends_with_glance_log() {
        let path = get_log_path();
        assert!(path.ends_with("glance.log"));
    }
}
