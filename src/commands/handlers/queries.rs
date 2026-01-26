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
        return CommandResult::error("Usage: /savequery <name> [description=...] [#tags...]");
    }

    // Prefer current_input (if non-SQL command input exists), otherwise use last_executed_sql
    let sql = match ctx.current_input {
        Some(input) if !input.is_empty() && !input.starts_with('/') => input.to_string(),
        _ => match ctx.last_executed_sql {
            Some(sql) => sql.to_string(),
            None => {
                return CommandResult::error("No query to save. Execute a query first.");
            }
        },
    };

    // Normalize tags: strip "global:" prefix for storage, but keep track of global scope
    let normalized_tags: Vec<String> = args
        .tags
        .iter()
        .map(|t| persistence::saved_queries::normalize_tag(t).to_string())
        .collect();

    // If any tag starts with "global:", treat this as a global query
    let has_global_tag = args
        .tags
        .iter()
        .any(|t| persistence::saved_queries::is_global_tag(t));
    let connection_scope = if has_global_tag {
        None
    } else {
        ctx.current_connection
    };

    // Check if query already exists
    let existing = persistence::saved_queries::get_saved_query_by_name(
        state_db.pool(),
        &args.name,
        connection_scope,
    )
    .await;

    match existing {
        Ok(Some(existing_query)) => {
            // Update existing query
            match persistence::saved_queries::update_saved_query(
                state_db.pool(),
                existing_query.id,
                Some(&sql),
                args.description.as_deref(),
                Some(&normalized_tags),
            )
            .await
            {
                Ok(()) => CommandResult::system(format!("Saved query '{}' updated.", args.name)),
                Err(e) => CommandResult::error(e.to_string()),
            }
        }
        Ok(None) | Err(_) => {
            // Create new query
            match persistence::saved_queries::create_saved_query(
                state_db.pool(),
                &args.name,
                &sql,
                args.description.as_deref(),
                connection_scope,
                &normalized_tags,
            )
            .await
            {
                Ok(_id) => CommandResult::system(format!("Query saved as '{}'.", args.name)),
                Err(e) => CommandResult::error(e.to_string()),
            }
        }
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
        tags: args.tag.clone().map(|t| vec![t]),
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

    let queries_text = queries
        .iter()
        .map(|query| {
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
            format!(
                "  • {} ({}){} - used {} times\n",
                query.name, scope, tags_str, query.usage_count
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let output = format!("Saved queries:\n{}", queries_text);

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
        saved_query_id: Some(query.id),
    }
}

/// Handle /query delete command.
pub async fn handle_query_delete(
    args: &crate::commands::router::QueryDeleteArgs,
    current_connection: Option<&str>,
    state_db: &Arc<StateDb>,
) -> CommandResult {
    if args.name.is_empty() {
        return CommandResult::error("Usage: /query delete <name> --confirm");
    }

    if !args.confirmed {
        return CommandResult::error(format!(
            "Delete saved query '{}'? Run with --confirm to proceed.",
            args.name
        ));
    }

    match persistence::saved_queries::delete_saved_query_by_name(
        state_db.pool(),
        &args.name,
        current_connection,
    )
    .await
    {
        Ok(()) => CommandResult::system(format!("Saved query '{}' deleted.", args.name)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}
