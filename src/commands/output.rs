//! Transport-agnostic command output types.
//!
//! These types represent command results in a way that is independent of the
//! presentation layer (TUI, CLI, HTTP API, etc.). Each transport layer can
//! convert these to its own representation.

use std::time::Duration;

/// Output from a command handler.
///
/// This enum represents all possible outputs from command execution in a
/// transport-agnostic way. The TUI, CLI, or API layers convert these to
/// their respective formats.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum CommandOutput {
    /// Informational message (success, status, etc.).
    Info(String),

    /// Error message.
    Error(String),

    /// Structured table data for display.
    Table {
        /// Column headers.
        headers: Vec<String>,
        /// Row data (each row is a vector of cell values).
        rows: Vec<Vec<String>>,
    },

    /// Query execution result with metadata.
    QueryResult {
        /// The SQL that was executed.
        sql: String,
        /// Number of rows affected or returned.
        row_count: usize,
        /// How long the query took.
        duration: Duration,
    },

    /// Application control action.
    Control(ControlAction),

    /// Multiple outputs (for commands that produce several messages).
    Multiple(Vec<CommandOutput>),
}

/// Control actions that affect application state.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ControlAction {
    /// Exit the application.
    Exit,

    /// Toggle vim-style navigation mode.
    ToggleVimMode,

    /// Clear conversation history.
    ClearConversation,

    /// Switch to a different database connection.
    SwitchConnection {
        /// Connection name.
        name: String,
        /// Connection display string (e.g., "user@host:port/db").
        connection_info: String,
    },

    /// Schema was refreshed.
    SchemaRefreshed {
        /// Number of tables found.
        table_count: usize,
    },
}

#[allow(dead_code)]
impl CommandOutput {
    /// Creates an info message.
    pub fn info(msg: impl Into<String>) -> Self {
        Self::Info(msg.into())
    }

    /// Creates an error message.
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    /// Creates a table output.
    pub fn table(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self::Table { headers, rows }
    }

    /// Creates a multiple output from a vector.
    pub fn multiple(outputs: Vec<CommandOutput>) -> Self {
        Self::Multiple(outputs)
    }

    /// Creates an exit control action.
    pub fn exit() -> Self {
        Self::Control(ControlAction::Exit)
    }

    /// Creates a toggle vim mode control action.
    pub fn toggle_vim_mode() -> Self {
        Self::Control(ControlAction::ToggleVimMode)
    }

    /// Creates a clear conversation control action.
    pub fn clear_conversation() -> Self {
        Self::Control(ControlAction::ClearConversation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_output() {
        let output = CommandOutput::info("Hello, world!");
        assert!(matches!(output, CommandOutput::Info(s) if s == "Hello, world!"));
    }

    #[test]
    fn test_error_output() {
        let output = CommandOutput::error("Something went wrong");
        assert!(matches!(output, CommandOutput::Error(s) if s == "Something went wrong"));
    }

    #[test]
    fn test_table_output() {
        let output = CommandOutput::table(
            vec!["Name".to_string(), "Age".to_string()],
            vec![vec!["Alice".to_string(), "30".to_string()]],
        );
        assert!(matches!(output, CommandOutput::Table { headers, rows }
            if headers.len() == 2 && rows.len() == 1));
    }

    #[test]
    fn test_control_actions() {
        assert!(matches!(
            CommandOutput::exit(),
            CommandOutput::Control(ControlAction::Exit)
        ));
        assert!(matches!(
            CommandOutput::toggle_vim_mode(),
            CommandOutput::Control(ControlAction::ToggleVimMode)
        ));
    }
}
