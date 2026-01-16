//! Error types for Glance.
//!
//! Defines the main error enum used throughout the application.

use thiserror::Error;

/// Main error type for Glance operations.
#[derive(Error, Debug)]
#[allow(dead_code)] // Variants will be used in later phases
pub enum GlanceError {
    /// Database connection errors (host unreachable, auth failed, etc.)
    #[error("Connection error: {0}")]
    Connection(String),

    /// Query execution errors (syntax errors, constraint violations, etc.)
    #[error("Query error: {0}")]
    Query(String),

    /// LLM API errors (rate limits, auth, timeouts, etc.)
    #[error("LLM error: {0}")]
    Llm(String),

    /// Configuration errors (invalid config file, missing required fields, etc.)
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal application errors (unexpected states, bugs, etc.)
    #[error("Internal error: {0}")]
    Internal(String),
}

#[allow(dead_code)] // Methods will be used in later phases
impl GlanceError {
    /// Creates a connection error with the given message.
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// Creates a query error with the given message.
    pub fn query(msg: impl Into<String>) -> Self {
        Self::Query(msg.into())
    }

    /// Creates an LLM error with the given message.
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::Llm(msg.into())
    }

    /// Creates a configuration error with the given message.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Creates an internal error with the given message.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Returns the error category as a string for display purposes.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Connection(_) => "Connection Error",
            Self::Query(_) => "Query Error",
            Self::Llm(_) => "LLM Error",
            Self::Config(_) => "Configuration Error",
            Self::Internal(_) => "Internal Error",
        }
    }
}

/// Result type alias using GlanceError.
pub type Result<T> = std::result::Result<T, GlanceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_connection() {
        let err = GlanceError::connection("Cannot connect to localhost:5432");
        assert_eq!(
            err.to_string(),
            "Connection error: Cannot connect to localhost:5432"
        );
        assert_eq!(err.category(), "Connection Error");
    }

    #[test]
    fn test_error_display_query() {
        let err = GlanceError::query("column \"emal\" does not exist");
        assert_eq!(
            err.to_string(),
            "Query error: column \"emal\" does not exist"
        );
        assert_eq!(err.category(), "Query Error");
    }

    #[test]
    fn test_error_display_llm() {
        let err = GlanceError::llm("Rate limited. Please wait.");
        assert_eq!(err.to_string(), "LLM error: Rate limited. Please wait.");
        assert_eq!(err.category(), "LLM Error");
    }

    #[test]
    fn test_error_display_config() {
        let err = GlanceError::config("missing field 'database' in connections.default");
        assert_eq!(
            err.to_string(),
            "Configuration error: missing field 'database' in connections.default"
        );
        assert_eq!(err.category(), "Configuration Error");
    }

    #[test]
    fn test_error_display_internal() {
        let err = GlanceError::internal("unexpected state");
        assert_eq!(err.to_string(), "Internal error: unexpected state");
        assert_eq!(err.category(), "Internal Error");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GlanceError>();
    }
}
