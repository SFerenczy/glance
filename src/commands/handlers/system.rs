//! System command handlers (/help, /clear, /schema, /quit, /vim).

use super::{CommandContext, CommandResult};
use crate::commands::help::HELP_TEXT;
use crate::tui::app::ChatMessage;

/// Handle /help command.
pub fn handle_help() -> CommandResult {
    CommandResult::Messages(vec![ChatMessage::System(HELP_TEXT.to_string())], None)
}

/// Handle /schema command.
pub fn handle_schema(ctx: &CommandContext<'_>) -> CommandResult {
    let schema_text = ctx.schema.format_for_display();
    CommandResult::Messages(vec![ChatMessage::System(schema_text)], None)
}

/// Handle /clear command.
pub fn handle_clear() -> CommandResult {
    CommandResult::Messages(
        vec![ChatMessage::System(
            "Chat history and context cleared.".to_string(),
        )],
        None,
    )
}

/// Handle /quit or /exit command.
pub fn handle_quit() -> CommandResult {
    CommandResult::Exit
}

/// Handle /vim command.
pub fn handle_vim() -> CommandResult {
    CommandResult::ToggleVimMode
}

/// Handle /rownumbers command.
pub fn handle_rownumbers() -> CommandResult {
    CommandResult::ToggleRowNumbers
}

/// Handle unknown command.
pub fn handle_unknown(command: &str) -> CommandResult {
    CommandResult::Messages(
        vec![ChatMessage::Error(format!(
            "Unknown command: {}. Type /help for available commands.",
            command
        ))],
        None,
    )
}

/// Handle /sql with empty args.
pub fn handle_sql_empty() -> CommandResult {
    CommandResult::Messages(
        vec![ChatMessage::Error("Usage: /sql <query>".to_string())],
        None,
    )
}
