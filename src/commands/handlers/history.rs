//! History command handlers (/history, /history clear).

use super::{CommandContext, CommandResult};
use crate::commands::router::HistoryArgs;
use crate::persistence::{self, HistoryFilter, QueryStatus};
use crate::tui::app::ChatMessage;

/// Handle /history command.
pub async fn handle_history(ctx: &CommandContext<'_>, args: &HistoryArgs) -> CommandResult {
    let state_db = match ctx.state_db {
        Some(db) => db,
        None => {
            return CommandResult::error("State database not available.");
        }
    };

    let filter = HistoryFilter {
        connection_name: args.connection.clone(),
        text_search: args.text.clone(),
        limit: args.limit.or(Some(20)),
        since_days: args.since_days,
    };

    let entries = match persistence::history::list_history(state_db.pool(), &filter).await {
        Ok(e) => e,
        Err(e) => return CommandResult::error(e.to_string()),
    };

    if entries.is_empty() {
        return CommandResult::system("No history entries found.");
    }

    let mut output = String::from("Query history:\n");
    for entry in &entries {
        let status_icon = match entry.status {
            QueryStatus::Success => "✓",
            QueryStatus::Error => "✗",
            QueryStatus::Cancelled => "○",
        };
        let sql_preview: String = entry.sql.chars().take(60).collect();
        let sql_preview = if entry.sql.len() > 60 {
            format!("{}...", sql_preview)
        } else {
            sql_preview
        };
        output.push_str(&format!(
            "  {} [{}] {}\n",
            status_icon,
            entry.created_at,
            sql_preview.replace('\n', " ")
        ));
    }

    CommandResult::Messages(
        vec![ChatMessage::System(output.trim_end().to_string())],
        None,
    )
}

/// Handle /history clear command.
pub async fn handle_history_clear(ctx: &CommandContext<'_>) -> CommandResult {
    let state_db = match ctx.state_db {
        Some(db) => db,
        None => {
            return CommandResult::error("State database not available.");
        }
    };

    match persistence::history::clear_history(state_db.pool()).await {
        Ok(count) => CommandResult::system(format!("Cleared {} history entries.", count)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}
