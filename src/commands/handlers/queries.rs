//! Saved queries command handlers (/savequery, /queries, /usequery, /query delete).

use std::sync::Arc;

use super::{CommandContext, CommandResult};
use crate::commands::router::{QueriesListArgs, SaveQueryArgs};
use crate::persistence::{self, SavedQueryFilter, StateDb};
use crate::tui::app::ChatMessage;

/// Handle /savequery command.
pub async fn handle_savequery(
    ctx: &CommandContext<'_>,
    args: &SaveQueryArgs,
    state_db: &Arc<StateDb>,
) -> CommandResult {
    if args.name.is_empty() {
        return CommandResult::error("Usage: /savequery <name> [#tags...]");
    }

    let sql = match ctx.last_executed_sql {
        Some(sql) => sql.to_string(),
        None => {
            return CommandResult::error("No query to save. Execute a query first.");
        }
    };

    match persistence::saved_queries::create_saved_query(
        state_db.pool(),
        &args.name,
        &sql,
        None,
        ctx.current_connection,
        &args.tags,
    )
    .await
    {
        Ok(_id) => CommandResult::system(format!("Query saved as '{}'.", args.name)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}

/// Handle /queries command.
pub async fn handle_queries_list(
    ctx: &CommandContext<'_>,
    args: &QueriesListArgs,
) -> CommandResult {
    let state_db = match ctx.state_db {
        Some(db) => db,
        None => {
            return CommandResult::error("State database not available.");
        }
    };

    let filter = SavedQueryFilter {
        connection_name: if args.all {
            None
        } else {
            args.connection
                .clone()
                .or_else(|| ctx.current_connection.map(|s| s.to_string()))
        },
        include_global: true,
        tag: args.tag.clone(),
        text_search: args.text.clone(),
        limit: None,
    };

    let queries =
        match persistence::saved_queries::list_saved_queries(state_db.pool(), &filter).await {
            Ok(q) => q,
            Err(e) => return CommandResult::error(e.to_string()),
        };

    if queries.is_empty() {
        return CommandResult::system("No saved queries found.");
    }

    let mut output = String::from("Saved queries:\n");
    for query in &queries {
        let tags_str = if query.tags.is_empty() {
            String::new()
        } else {
            format!(
                " [{}]",
                query
                    .tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let scope = query.connection_name.as_deref().unwrap_or("global");
        output.push_str(&format!(
            "  • {} ({}){} - used {} times\n",
            query.name, scope, tags_str, query.usage_count
        ));
    }

    CommandResult::Messages(
        vec![ChatMessage::System(output.trim_end().to_string())],
        None,
    )
}

/// Handle /usequery command.
pub async fn handle_usequery(
    name: &str,
    current_connection: Option<&str>,
    state_db: &Arc<StateDb>,
) -> CommandResult {
    if name.is_empty() {
        return CommandResult::error("Usage: /usequery <name>");
    }

    let query = match persistence::saved_queries::get_saved_query_by_name(
        state_db.pool(),
        name,
        current_connection,
    )
    .await
    {
        Ok(Some(q)) => q,
        Ok(None) => {
            return CommandResult::error(format!("Saved query '{}' not found.", name));
        }
        Err(e) => return CommandResult::error(e.to_string()),
    };

    if let Err(e) = persistence::saved_queries::record_usage(state_db.pool(), query.id).await {
        tracing::warn!("Failed to record query usage: {}", e);
    }

    // Insert the SQL into the input bar prefixed with /sql
    CommandResult::SetInput {
        content: format!("/sql {}", query.sql),
        message: Some(ChatMessage::System(format!(
            "Loaded query '{}' into input. Press Enter to execute.",
            query.name
        ))),
    }
}

/// Handle /query delete command.
pub async fn handle_query_delete(
    name: &str,
    current_connection: Option<&str>,
    state_db: &Arc<StateDb>,
) -> CommandResult {
    if name.is_empty() {
        return CommandResult::error("Usage: /query delete <name>");
    }

    match persistence::saved_queries::delete_saved_query_by_name(
        state_db.pool(),
        name,
        current_connection,
    )
    .await
    {
        Ok(()) => CommandResult::system(format!("Saved query '{}' deleted.", name)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}
