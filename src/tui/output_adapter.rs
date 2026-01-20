//! TUI output adapter for converting CommandOutput to ChatMessage.
//!
//! This module provides the conversion from transport-agnostic CommandOutput
//! to TUI-specific ChatMessage types.

use crate::commands::output::{CommandOutput, ControlAction};
use crate::tui::app::ChatMessage;

/// Converts a CommandOutput to a vector of ChatMessages for the TUI.
#[allow(dead_code)]
pub fn to_chat_messages(output: CommandOutput) -> Vec<ChatMessage> {
    match output {
        CommandOutput::Info(msg) => vec![ChatMessage::System(msg)],
        CommandOutput::Error(msg) => vec![ChatMessage::Error(msg)],
        CommandOutput::Table { headers, rows } => {
            let formatted = format_table(&headers, &rows);
            vec![ChatMessage::System(formatted)]
        }
        CommandOutput::QueryResult {
            sql: _,
            row_count,
            duration,
        } => {
            vec![ChatMessage::System(format!(
                "Query executed in {:?}. {} row(s) affected.",
                duration, row_count
            ))]
        }
        CommandOutput::Control(action) => control_action_to_messages(action),
        CommandOutput::Multiple(outputs) => {
            outputs.into_iter().flat_map(to_chat_messages).collect()
        }
    }
}

/// Converts a ControlAction to messages (some control actions have associated messages).
fn control_action_to_messages(action: ControlAction) -> Vec<ChatMessage> {
    match action {
        ControlAction::Exit => vec![],
        ControlAction::ToggleVimMode => vec![],
        ControlAction::ClearConversation => {
            vec![ChatMessage::System(
                "Chat history and context cleared.".to_string(),
            )]
        }
        ControlAction::SwitchConnection {
            name,
            connection_info,
        } => {
            vec![ChatMessage::System(format!(
                "Connected to '{}' ({}).",
                name, connection_info
            ))]
        }
        ControlAction::SchemaRefreshed { table_count } => {
            vec![ChatMessage::System(format!(
                "Schema refreshed. Found {} tables.",
                table_count
            ))]
        }
    }
}

/// Formats a table as a string for display.
fn format_table(headers: &[String], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut output = String::new();

    // Header row
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h, width = widths[i]))
        .collect();
    output.push_str(&header_line.join(" │ "));
    output.push('\n');

    // Separator
    let separator: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    output.push_str(&separator.join("─┼─"));
    output.push('\n');

    // Data rows
    for row in rows {
        let row_line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths.get(i).copied().unwrap_or(cell.len());
                format!("{:width$}", cell, width = width)
            })
            .collect();
        output.push_str(&row_line.join(" │ "));
        output.push('\n');
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_to_chat_message() {
        let output = CommandOutput::info("Test message");
        let messages = to_chat_messages(output);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0], ChatMessage::System(s) if s == "Test message"));
    }

    #[test]
    fn test_error_to_chat_message() {
        let output = CommandOutput::error("Error occurred");
        let messages = to_chat_messages(output);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0], ChatMessage::Error(s) if s == "Error occurred"));
    }

    #[test]
    fn test_table_formatting() {
        let output = CommandOutput::table(
            vec!["Name".to_string(), "Age".to_string()],
            vec![
                vec!["Alice".to_string(), "30".to_string()],
                vec!["Bob".to_string(), "25".to_string()],
            ],
        );
        let messages = to_chat_messages(output);
        assert_eq!(messages.len(), 1);
        if let ChatMessage::System(s) = &messages[0] {
            assert!(s.contains("Name"));
            assert!(s.contains("Alice"));
            assert!(s.contains("Bob"));
        } else {
            panic!("Expected System message");
        }
    }

    #[test]
    fn test_multiple_outputs() {
        let output = CommandOutput::multiple(vec![
            CommandOutput::info("First"),
            CommandOutput::info("Second"),
        ]);
        let messages = to_chat_messages(output);
        assert_eq!(messages.len(), 2);
    }
}
