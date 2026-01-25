//! LLM service for orchestrating natural language to SQL conversion.
//!
//! This module provides the unified NL→SQL pipeline used by both TUI and headless modes.
//! It encapsulates LLM interaction, prompt building, tool handling, and response parsing.
//!
//! # Architecture
//!
//! The `LlmService` is the single entry point for all natural language processing:
//! - TUI mode: `Orchestrator::handle_input()` → `LlmService::process_query()`
//! - Headless mode: `HeadlessRunner` → `Orchestrator::handle_input()` → `LlmService::process_query()`
//!
//! This ensures identical behavior and logging across all execution modes.

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use crate::db::Schema;
use crate::error::Result;
use crate::persistence::{self, SavedQueryFilter, StateDb};
use futures::StreamExt;

use super::{
    build_messages_cached, format_saved_queries_for_llm, get_tool_definitions, parse_llm_response,
    prompt::ConnectionContext, Conversation, ListSavedQueriesInput, LlmClient, LlmResponse,
    PromptCache, ToolResult,
};

/// LLM service that handles natural language processing and tool calls.
pub struct LlmService {
    client: Box<dyn LlmClient>,
    prompt_cache: PromptCache,
}

/// Context for tool execution.
pub struct ToolContext<'a> {
    /// State database for persistence.
    pub state_db: Option<&'a Arc<StateDb>>,
    /// Current connection name.
    pub current_connection: Option<&'a str>,
}

/// Result of LLM processing.
#[derive(Debug)]
pub enum LlmResult {
    /// SQL was generated, with optional explanation.
    Sql {
        sql: String,
        explanation: Option<String>,
    },
    /// No SQL, just explanatory text.
    Explanation(String),
}

impl LlmService {
    /// Creates a new LLM service.
    pub fn new(client: Box<dyn LlmClient>) -> Self {
        Self {
            client,
            prompt_cache: PromptCache::new(),
        }
    }

    /// Process natural language input and return SQL or explanation.
    ///
    /// This is the unified entry point for all NL→SQL processing, used by both
    /// TUI and headless modes. It handles:
    /// - Prompt building with caching
    /// - LLM completion with tool support
    /// - Tool execution (e.g., list_saved_queries)
    /// - Response parsing to extract SQL
    ///
    /// All processing is logged via tracing for observability.
    pub async fn process_query(
        &mut self,
        input: &str,
        schema: &Schema,
        conversation: &mut Conversation,
        tool_context: &ToolContext<'_>,
    ) -> Result<LlmResult> {
        let start = Instant::now();
        tracing::debug!(input_len = input.len(), "Starting NL→SQL processing");

        conversation.add_user(input);

        // Build redacted connection context for LLM prompt
        let connection_ctx = self.build_connection_context(tool_context).await;

        let messages = build_messages_cached(
            &mut self.prompt_cache,
            schema,
            conversation,
            &connection_ctx,
        );
        let tools = get_tool_definitions();

        tracing::debug!(
            message_count = messages.len(),
            tool_count = tools.len(),
            "Sending request to LLM"
        );

        let llm_start = Instant::now();
        let mut response = self.client.complete_with_tools(&messages, &tools).await?;
        let llm_duration = llm_start.elapsed();

        tracing::debug!(
            llm_duration_ms = llm_duration.as_millis(),
            has_tool_calls = response.has_tool_calls(),
            response_len = response.content.len(),
            "Received LLM response"
        );

        if response.has_tool_calls() {
            let tool_count = response.tool_calls.len();
            tracing::debug!(tool_count, "Processing tool calls");
            response = self
                .handle_tool_calls(response, &tools, schema, conversation, tool_context)
                .await?;
        }

        conversation.add_assistant(response.content.as_str());

        let parsed = parse_llm_response(&response.content);
        let total_duration = start.elapsed();

        if let Some(ref sql) = parsed.sql {
            tracing::info!(
                total_duration_ms = total_duration.as_millis(),
                sql_len = sql.len(),
                has_explanation = !parsed.text.is_empty(),
                "NL→SQL processing complete: generated SQL"
            );
            Ok(LlmResult::Sql {
                sql: sql.clone(),
                explanation: if parsed.text.is_empty() {
                    None
                } else {
                    Some(parsed.text)
                },
            })
        } else {
            tracing::info!(
                total_duration_ms = total_duration.as_millis(),
                explanation_len = parsed.text.len(),
                "NL→SQL processing complete: explanation only"
            );
            Ok(LlmResult::Explanation(parsed.text))
        }
    }

    /// Process natural language input with streaming output support.
    pub async fn process_query_streaming<F, Fut>(
        &mut self,
        input: &str,
        schema: &Schema,
        conversation: &mut Conversation,
        tool_context: &ToolContext<'_>,
        mut on_token: F,
    ) -> Result<LlmResult>
    where
        F: FnMut(&str) -> Fut,
        Fut: Future<Output = ()>,
    {
        let start = Instant::now();
        tracing::debug!(
            input_len = input.len(),
            "Starting streaming NL→SQL processing"
        );

        conversation.add_user(input);

        // Build redacted connection context for LLM prompt
        let connection_ctx = self.build_connection_context(tool_context).await;

        let messages = build_messages_cached(
            &mut self.prompt_cache,
            schema,
            conversation,
            &connection_ctx,
        );
        let tools = get_tool_definitions();

        tracing::debug!(
            message_count = messages.len(),
            tool_count = tools.len(),
            "Sending streaming request to LLM"
        );

        let llm_start = Instant::now();
        let mut response_content = String::new();
        let stream_result = self.client.complete_stream(&messages).await;

        match stream_result {
            Ok(mut stream) => {
                while let Some(chunk) = stream.next().await {
                    let token = chunk?;
                    response_content.push_str(&token);
                    on_token(&token).await;
                }
            }
            Err(err) => {
                tracing::warn!(
                    "Streaming unavailable, falling back to non-streaming: {}",
                    err
                );
                let mut response = self.client.complete_with_tools(&messages, &tools).await?;
                if response.has_tool_calls() {
                    response = self
                        .handle_tool_calls(response, &tools, schema, conversation, tool_context)
                        .await?;
                }
                response_content = response.content;
            }
        }

        let llm_duration = llm_start.elapsed();
        tracing::debug!(
            llm_duration_ms = llm_duration.as_millis(),
            response_len = response_content.len(),
            "Received streaming LLM response"
        );

        conversation.add_assistant(response_content.as_str());

        let parsed = parse_llm_response(&response_content);
        let total_duration = start.elapsed();

        if let Some(ref sql) = parsed.sql {
            tracing::info!(
                total_duration_ms = total_duration.as_millis(),
                sql_len = sql.len(),
                has_explanation = !parsed.text.is_empty(),
                "Streaming NL→SQL processing complete: generated SQL"
            );
            Ok(LlmResult::Sql {
                sql: sql.clone(),
                explanation: if parsed.text.is_empty() {
                    None
                } else {
                    Some(parsed.text)
                },
            })
        } else {
            tracing::info!(
                total_duration_ms = total_duration.as_millis(),
                explanation_len = parsed.text.len(),
                "Streaming NL→SQL processing complete: explanation only"
            );
            Ok(LlmResult::Explanation(parsed.text))
        }
    }

    /// Handle tool calls from the LLM and return the final response.
    async fn handle_tool_calls(
        &mut self,
        response: LlmResponse,
        tools: &[super::ToolDefinition],
        schema: &Schema,
        conversation: &Conversation,
        tool_context: &ToolContext<'_>,
    ) -> Result<LlmResponse> {
        let mut tool_results = Vec::new();

        for tool_call in &response.tool_calls {
            let result = self
                .execute_tool(&tool_call.name, &tool_call.arguments, tool_context)
                .await;
            tool_results.push(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: result,
            });
        }

        // Build redacted connection context for LLM prompt
        let connection_ctx = self.build_connection_context(tool_context).await;

        let messages = build_messages_cached(
            &mut self.prompt_cache,
            schema,
            conversation,
            &connection_ctx,
        );

        self.client
            .continue_with_tool_results(&messages, &response.tool_calls, &tool_results, tools)
            .await
    }

    /// Execute a tool and return the result as JSON string.
    async fn execute_tool(
        &self,
        name: &str,
        arguments: &str,
        tool_context: &ToolContext<'_>,
    ) -> String {
        let start = Instant::now();
        tracing::debug!(tool_name = name, "Executing tool");

        let result = match name {
            "list_saved_queries" => {
                self.execute_list_saved_queries(arguments, tool_context)
                    .await
            }
            _ => {
                tracing::warn!(tool_name = name, "Unknown tool requested");
                serde_json::json!({
                    "error": format!("Unknown tool: {}", name)
                })
                .to_string()
            }
        };

        tracing::debug!(
            tool_name = name,
            duration_ms = start.elapsed().as_millis(),
            result_len = result.len(),
            "Tool execution complete"
        );

        result
    }

    /// Execute the list_saved_queries tool.
    async fn execute_list_saved_queries(
        &self,
        arguments: &str,
        tool_context: &ToolContext<'_>,
    ) -> String {
        let state_db = match tool_context.state_db {
            Some(db) => db,
            None => {
                return serde_json::json!({
                    "error": "State database not available"
                })
                .to_string();
            }
        };

        let input: ListSavedQueriesInput = match serde_json::from_str(arguments) {
            Ok(input) => input,
            Err(_) => ListSavedQueriesInput {
                connection_name: None,
                tags: None,
                text: None,
                limit: None,
            },
        };

        let filter = SavedQueryFilter {
            connection_name: input
                .connection_name
                .or_else(|| tool_context.current_connection.map(|s| s.to_string())),
            include_global: true,
            tags: input.tags,
            text_search: input.text,
            limit: input.limit,
        };

        match persistence::saved_queries::list_saved_queries(state_db.pool(), &filter).await {
            Ok(queries) => {
                let output = format_saved_queries_for_llm(&queries);
                serde_json::to_string(&output).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => serde_json::json!({
                "error": format!("Failed to list saved queries: {}", e)
            })
            .to_string(),
        }
    }

    /// Builds a redacted connection context for the LLM prompt.
    ///
    /// Retrieves the database name from the connection profile if available,
    /// using only the connection name (label) and database name (never host, user, or password).
    async fn build_connection_context(&self, tool_context: &ToolContext<'_>) -> ConnectionContext {
        let label = tool_context.current_connection.map(|s| s.to_string());

        // Attempt to retrieve database name from connection profile
        let database = if let (Some(state_db), Some(conn_name)) =
            (tool_context.state_db, tool_context.current_connection)
        {
            persistence::connections::get_connection(state_db.pool(), conn_name)
                .await
                .ok()
                .flatten()
                .map(|profile| profile.database)
        } else {
            None
        };

        ConnectionContext::new(label, database)
    }

    /// Returns a reference to the underlying LLM client.
    pub fn client(&self) -> &dyn LlmClient {
        self.client.as_ref()
    }

    /// Replaces the LLM client (e.g., after provider change).
    pub fn set_client(&mut self, client: Box<dyn LlmClient>) {
        self.client = client;
    }

    /// Invalidates the prompt cache (e.g., after schema refresh).
    pub fn invalidate_cache(&mut self) {
        self.prompt_cache.invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Column, Table};
    use crate::llm::MockLlmClient;

    fn sample_schema() -> Schema {
        Schema {
            tables: vec![Table {
                name: "users".to_string(),
                columns: vec![
                    Column::new("id", "integer"),
                    Column::new("name", "varchar(255)"),
                ],
                primary_key: vec!["id".to_string()],
                indexes: vec![],
            }],
            foreign_keys: vec![],
        }
    }

    #[tokio::test]
    async fn test_process_query_returns_sql() {
        let mut service = LlmService::new(Box::new(MockLlmClient::new()));
        let schema = sample_schema();
        let mut conversation = Conversation::new();
        let tool_context = ToolContext {
            state_db: None,
            current_connection: None,
        };

        let result = service
            .process_query(
                "show me all users",
                &schema,
                &mut conversation,
                &tool_context,
            )
            .await
            .unwrap();

        match result {
            LlmResult::Sql { sql, .. } => {
                assert!(sql.to_uppercase().contains("SELECT"));
            }
            LlmResult::Explanation(_) => panic!("Expected SQL result"),
        }
    }

    #[tokio::test]
    async fn test_conversation_updated() {
        let mut service = LlmService::new(Box::new(MockLlmClient::new()));
        let schema = sample_schema();
        let mut conversation = Conversation::new();
        let tool_context = ToolContext {
            state_db: None,
            current_connection: None,
        };

        assert!(conversation.is_empty());

        let _ = service
            .process_query(
                "show me all users",
                &schema,
                &mut conversation,
                &tool_context,
            )
            .await;

        assert!(!conversation.is_empty());
    }
}
