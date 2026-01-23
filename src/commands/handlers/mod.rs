//! Command handlers for Glance.
//!
//! Each handler is a pure function that takes a command context and returns a result.

pub mod connection;
pub mod history;
pub mod llm_settings;
pub mod queries;
pub mod system;

use std::sync::Arc;

use crate::db::{DatabaseClient, Schema};
use crate::persistence::StateDb;
use crate::safety::ClassificationResult;
use crate::tui::app::{ChatMessage, QueryLogEntry};

/// Context provided to command handlers.
pub struct CommandContext<'a> {
    /// Database client for executing queries.
    #[allow(dead_code)]
    pub db: Option<&'a dyn DatabaseClient>,
    /// State database for persistence.
    pub state_db: Option<&'a Arc<StateDb>>,
    /// Database schema.
    pub schema: &'a Schema,
    /// Current connection name.
    pub current_connection: Option<&'a str>,
    /// Last executed SQL (for /savequery).
    pub last_executed_sql: Option<&'a str>,
    /// Current input text (for /savequery when input is non-empty).
    pub current_input: Option<&'a str>,
}

/// Result of executing a command.
#[derive(Debug)]
#[allow(dead_code)]
pub enum CommandResult {
    /// Messages to add to the chat, with an optional query log entry.
    Messages(Vec<ChatMessage>, Option<QueryLogEntry>),
    /// A query needs confirmation before execution.
    NeedsConfirmation {
        sql: String,
        classification: ClassificationResult,
    },
    /// Application should exit.
    Exit,
    /// Toggle vim mode.
    ToggleVimMode,
    /// Toggle row numbers in result tables.
    ToggleRowNumbers,
    /// Connection switched successfully.
    ConnectionSwitch {
        /// Messages to display.
        messages: Vec<ChatMessage>,
        /// New connection display string.
        connection_info: String,
        /// Database schema for SQL completions.
        schema: Schema,
    },
    /// Schema was refreshed successfully.
    SchemaRefresh {
        /// Messages to display.
        messages: Vec<ChatMessage>,
        /// Updated database schema.
        schema: Schema,
    },
    /// Set the input bar content (e.g., for /usequery).
    SetInput {
        /// Content to set in the input bar.
        content: String,
        /// Optional message to display.
        message: Option<ChatMessage>,
    },
    /// No action needed.
    None,
}

impl CommandResult {
    /// Creates a single system message result.
    pub fn system(msg: impl Into<String>) -> Self {
        Self::Messages(vec![ChatMessage::System(msg.into())], None)
    }

    /// Creates a single error message result.
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Messages(vec![ChatMessage::Error(msg.into())], None)
    }
}
