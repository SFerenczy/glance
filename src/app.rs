//! Core orchestrator for Glance.
//!
//! Coordinates the database client, LLM client, safety classifier,
//! and application state to implement the main chat loop.

use std::sync::Arc;
use std::time::Instant;

/// Helper macro to extract state_db or return an error InputResult.
macro_rules! require_state_db {
    ($self:expr) => {
        match &$self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "State database not available.".to_string(),
                    )],
                    None,
                ))
            }
        }
    };
}

use crate::commands::{
    handlers::{
        connection, history, llm_settings, queries, system,
        system::{
            handle_clear, handle_help, handle_quit, handle_schema, handle_sql_empty,
            handle_unknown, handle_vim,
        },
        CommandContext, CommandResult,
    },
    router::{LlmKeyArgs, LlmProviderArgs},
    Command, CommandRouter,
};
use crate::config::ConnectionConfig;
use crate::db::{DatabaseClient, QueryResult, Schema};
use crate::error::{GlanceError, Result};
use crate::llm::{
    Conversation, LlmClient, LlmProvider, LlmResult, LlmService, MockLlmClient, ToolContext,
};
use crate::persistence::{self, QueryStatus, SecretStorageStatus, StateDb, SubmittedBy};
use crate::safety::{classify_sql, ClassificationResult, SafetyLevel};
use crate::tui::app::{ChatMessage, QueryLogEntry, QuerySource};

/// Result of processing user input.
#[derive(Debug, Clone)]
pub enum InputResult {
    /// No action needed (empty input, etc.)
    None,
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
        /// Messages to display (e.g., "Connected to X").
        messages: Vec<ChatMessage>,
        /// New connection display string for the header.
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
}

/// The main orchestrator that coordinates all components.
pub struct Orchestrator {
    /// Database client for executing queries.
    db: Option<Box<dyn DatabaseClient>>,
    /// LLM service for NL→SQL conversion.
    llm_service: LlmService,
    /// Database schema for LLM context.
    schema: Schema,
    /// Conversation history for LLM context.
    conversation: Conversation,
    /// State database for persistence.
    state_db: Option<Arc<StateDb>>,
    /// Current connection name (if using a saved connection).
    current_connection_name: Option<String>,
    /// Last executed SQL (for /savequery).
    last_executed_sql: Option<String>,
}

impl Orchestrator {
    /// Creates a new orchestrator with the given components.
    #[allow(dead_code)]
    pub fn new(
        db: Option<Box<dyn DatabaseClient>>,
        llm: Box<dyn LlmClient>,
        schema: Schema,
    ) -> Self {
        Self {
            db,
            llm_service: LlmService::new(llm),
            schema,
            conversation: Conversation::new(),
            state_db: None,
            current_connection_name: None,
            last_executed_sql: None,
        }
    }

    /// Rebuilds the LLM client with current settings from persistence.
    async fn rebuild_llm_client(&mut self) -> Result<()> {
        if let Some(ref state_db) = self.state_db {
            let settings = persistence::llm_settings::get_llm_settings(state_db.pool()).await?;
            let provider = settings.provider.parse::<LlmProvider>().unwrap_or_default();
            let client =
                crate::llm::create_client_from_persistence(provider, Some(state_db)).await?;
            self.llm_service.set_client(client);
        }
        Ok(())
    }

    /// Creates an orchestrator by connecting to the database and initializing components.
    pub async fn connect(connection: &ConnectionConfig, llm_provider: LlmProvider) -> Result<Self> {
        // Connect to database using the factory
        let db = crate::db::connect(connection).await?;

        // Introspect schema
        let schema = db.introspect_schema().await?;

        // Open state database first so we can use persisted API key
        let state_db = StateDb::open_default().await.ok().map(Arc::new);

        // Create LLM client (using persisted key if available)
        let llm =
            crate::llm::create_client_from_persistence(llm_provider, state_db.as_ref()).await?;

        Ok(Self {
            db: Some(db),
            llm_service: LlmService::new(llm),
            schema,
            conversation: Conversation::new(),
            state_db,
            current_connection_name: None,
            last_executed_sql: None,
        })
    }

    /// Creates an orchestrator with a mock LLM for testing.
    #[allow(dead_code)]
    pub fn with_mock_llm(db: Option<Box<dyn DatabaseClient>>, schema: Schema) -> Self {
        Self {
            db,
            llm_service: LlmService::new(Box::new(MockLlmClient::new())),
            schema,
            state_db: None,
            current_connection_name: None,
            last_executed_sql: None,
            conversation: Conversation::new(),
        }
    }

    /// Creates an orchestrator with a mock LLM and state database for testing.
    #[allow(dead_code)]
    pub fn with_mock_llm_and_state_db(
        db: Option<Box<dyn DatabaseClient>>,
        schema: Schema,
        state_db: Arc<StateDb>,
    ) -> Self {
        Self {
            db,
            llm_service: LlmService::new(Box::new(MockLlmClient::new())),
            schema,
            state_db: Some(state_db),
            current_connection_name: Some("test".to_string()),
            last_executed_sql: None,
            conversation: Conversation::new(),
        }
    }

    /// Creates a fully mocked orchestrator for headless testing.
    #[allow(dead_code)]
    pub async fn for_headless_testing(state_db: Arc<StateDb>) -> Self {
        use crate::db::{Column, MockDatabaseClient, Table};

        // Create a test connection entry in the database to satisfy foreign key constraints
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO connections (name, database) VALUES ('test', 'testdb')",
        )
        .execute(state_db.pool())
        .await;

        // Create a test schema with sample tables for SQL completion testing
        let schema = Schema {
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("name", "varchar(255)"),
                        Column::new("email", "varchar(255)"),
                        Column::new("created_at", "timestamp"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("user_id", "integer"),
                        Column::new("total", "decimal"),
                        Column::new("status", "varchar(50)"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "products".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("name", "varchar(255)"),
                        Column::new("price", "decimal"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
            ],
            foreign_keys: vec![],
        };

        Self {
            db: Some(Box::new(MockDatabaseClient::new())),
            llm_service: LlmService::new(Box::new(MockLlmClient::new())),
            schema,
            state_db: Some(state_db),
            current_connection_name: Some("test".to_string()),
            last_executed_sql: None,
            conversation: Conversation::new(),
        }
    }

    /// Returns a reference to the database schema.
    #[allow(dead_code)]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Handles user input and returns the result.
    pub async fn handle_input(&mut self, input: &str) -> Result<InputResult> {
        let input = input.trim();

        if input.is_empty() {
            return Ok(InputResult::None);
        }

        // Check for commands
        if input.starts_with('/') {
            return self.handle_command(input).await;
        }

        // Natural language query - send to LLM
        self.handle_natural_language(input).await
    }

    /// Handles a command (input starting with /).
    async fn handle_command(&mut self, input: &str) -> Result<InputResult> {
        let command = CommandRouter::parse(input);

        // Build command context
        let ctx = CommandContext {
            db: self.db.as_deref(),
            state_db: self.state_db.as_ref(),
            schema: &self.schema,
            current_connection: self.current_connection_name.as_deref(),
            last_executed_sql: self.last_executed_sql.as_deref(),
            current_input: None, // Commands don't have access to prior input state
        };

        let result = match command {
            Command::Sql(sql) => {
                if sql.is_empty() {
                    handle_sql_empty()
                } else {
                    return self.handle_sql(&sql).await;
                }
            }
            Command::Clear => {
                self.conversation.clear();
                handle_clear()
            }
            Command::Schema => handle_schema(&ctx),
            Command::Quit => handle_quit(),
            Command::Vim => handle_vim(),
            Command::RowNumbers => system::handle_rownumbers(),
            Command::Help => handle_help(),
            Command::ConnectionsList => connection::handle_connections_list(&ctx).await,
            Command::Connect(name) => {
                return self.handle_connect(&name).await;
            }
            Command::ConnectionAdd(args) => {
                let state_db = require_state_db!(self);
                connection::handle_conn_add(&args, &state_db).await
            }
            Command::ConnectionEdit(args) => {
                let state_db = require_state_db!(self);
                connection::handle_conn_edit(&args, &state_db).await
            }
            Command::ConnectionDelete(args) => {
                let state_db = require_state_db!(self);
                connection::handle_conn_delete(&args, &state_db).await
            }
            Command::History(args) => history::handle_history(&ctx, &args).await,
            Command::HistoryClear { .. } => history::handle_history_clear(&ctx).await,
            Command::SaveQuery(args) => {
                let state_db = require_state_db!(self);
                queries::handle_savequery(&ctx, &args, &state_db).await
            }
            Command::QueriesList(args) => queries::handle_queries_list(&ctx, &args).await,
            Command::UseQuery(name) => {
                let state_db = require_state_db!(self);
                queries::handle_usequery(&name, self.current_connection_name.as_deref(), &state_db)
                    .await
            }
            Command::QueryDelete(name) => {
                let state_db = require_state_db!(self);
                queries::handle_query_delete(
                    &name,
                    self.current_connection_name.as_deref(),
                    &state_db,
                )
                .await
            }
            Command::LlmProvider(args) => {
                return self.handle_llm_provider(&args).await;
            }
            Command::LlmModel(args) => {
                return self.handle_llm_model(&args).await;
            }
            Command::LlmKey(args) => {
                return self.handle_llm_key(&args).await;
            }
            Command::LlmSettings => {
                let state_db = require_state_db!(self);
                llm_settings::handle_llm_settings(&state_db).await
            }
            Command::RefreshSchema => {
                return self.handle_refresh_schema().await;
            }
            Command::NaturalLanguage(_) => {
                // This shouldn't happen since we check for '/' prefix first
                return self.handle_natural_language(input).await;
            }
            Command::Unknown(cmd) => handle_unknown(&cmd),
        };

        Ok(self.command_result_to_input_result(result))
    }

    /// Converts a CommandResult to an InputResult.
    fn command_result_to_input_result(&self, result: CommandResult) -> InputResult {
        match result {
            CommandResult::Messages(msgs, entry) => InputResult::Messages(msgs, entry),
            CommandResult::NeedsConfirmation {
                sql,
                classification,
            } => InputResult::NeedsConfirmation {
                sql,
                classification,
            },
            CommandResult::Exit => InputResult::Exit,
            CommandResult::ToggleVimMode => InputResult::ToggleVimMode,
            CommandResult::ToggleRowNumbers => InputResult::ToggleRowNumbers,
            CommandResult::ConnectionSwitch {
                messages,
                connection_info,
                schema,
            } => InputResult::ConnectionSwitch {
                messages,
                connection_info,
                schema,
            },
            CommandResult::SchemaRefresh { messages, schema } => {
                InputResult::SchemaRefresh { messages, schema }
            }
            CommandResult::SetInput { content, message } => {
                InputResult::SetInput { content, message }
            }
            CommandResult::None => InputResult::None,
        }
    }

    /// Handles /refresh schema command.
    async fn handle_refresh_schema(&mut self) -> Result<InputResult> {
        let db = match &self.db {
            Some(db) => db,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "Not connected to a database.".to_string(),
                    )],
                    None,
                ))
            }
        };

        let schema = db.introspect_schema().await?;
        self.schema = schema.clone();
        self.llm_service.invalidate_cache();

        Ok(InputResult::SchemaRefresh {
            messages: vec![ChatMessage::System(format!(
                "Schema refreshed. Found {} tables.",
                schema.tables.len()
            ))],
            schema,
        })
    }

    /// Handles /llm provider command with LLM client rebuild.
    async fn handle_llm_provider(&mut self, args: &LlmProviderArgs) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "State database not available.".to_string(),
                    )],
                    None,
                ))
            }
        };

        match args {
            LlmProviderArgs::Show => {
                let result = llm_settings::handle_llm_provider(args, &state_db).await;
                Ok(self.command_result_to_input_result(result))
            }
            LlmProviderArgs::Set(value) => {
                match persistence::llm_settings::set_provider(state_db.pool(), value).await {
                    Ok(()) => {
                        self.conversation.clear();
                        if let Err(e) = self.rebuild_llm_client().await {
                            return Ok(InputResult::Messages(
                                vec![
                                    ChatMessage::System(format!(
                                        "LLM provider set to '{}'. Conversation cleared.",
                                        value
                                    )),
                                    ChatMessage::Error(format!(
                                        "Warning: Could not initialize LLM client: {}",
                                        e
                                    )),
                                ],
                                None,
                            ));
                        }
                        Ok(InputResult::Messages(
                            vec![ChatMessage::System(format!(
                                "LLM provider set to '{}'. Conversation cleared.",
                                value
                            ))],
                            None,
                        ))
                    }
                    Err(e) => Ok(InputResult::Messages(
                        vec![ChatMessage::Error(e.to_string())],
                        None,
                    )),
                }
            }
        }
    }

    /// Handles /llm key command with LLM client rebuild.
    async fn handle_llm_key(&mut self, args: &LlmKeyArgs) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "State database not available.".to_string(),
                    )],
                    None,
                ))
            }
        };

        match args {
            LlmKeyArgs::Show => {
                let result = llm_settings::handle_llm_key(args, &state_db).await;
                Ok(self.command_result_to_input_result(result))
            }
            LlmKeyArgs::Set(value) => {
                let provider =
                    match persistence::llm_settings::get_llm_settings(state_db.pool()).await {
                        Ok(s) => s.provider,
                        Err(e) => {
                            return Ok(InputResult::Messages(
                                vec![ChatMessage::Error(e.to_string())],
                                None,
                            ))
                        }
                    };
                match persistence::llm_settings::set_api_key(
                    state_db.pool(),
                    &provider,
                    value,
                    state_db.secrets(),
                )
                .await
                {
                    Ok(()) => {
                        let masked = persistence::SecretStorage::mask_secret(value);
                        if let Err(e) = self.rebuild_llm_client().await {
                            return Ok(InputResult::Messages(
                                vec![
                                    ChatMessage::System(format!(
                                        "API key set for provider '{}': {}",
                                        provider, masked
                                    )),
                                    ChatMessage::Error(format!(
                                        "Warning: Could not initialize LLM client: {}",
                                        e
                                    )),
                                ],
                                None,
                            ));
                        }
                        Ok(InputResult::Messages(
                            vec![ChatMessage::System(format!(
                                "API key set for provider '{}': {}",
                                provider, masked
                            ))],
                            None,
                        ))
                    }
                    Err(e) => Ok(InputResult::Messages(
                        vec![ChatMessage::Error(e.to_string())],
                        None,
                    )),
                }
            }
        }
    }

    /// Handles /llm model command with LLM client rebuild.
    async fn handle_llm_model(
        &mut self,
        args: &crate::commands::router::LlmModelArgs,
    ) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "State database not available.".to_string(),
                    )],
                    None,
                ))
            }
        };

        match args {
            crate::commands::router::LlmModelArgs::Show => {
                let result = llm_settings::handle_llm_model(args, &state_db).await;
                Ok(self.command_result_to_input_result(result))
            }
            crate::commands::router::LlmModelArgs::Set(value) => {
                match persistence::llm_settings::set_model(state_db.pool(), value).await {
                    Ok(()) => {
                        self.llm_service.invalidate_cache();
                        if let Err(e) = self.rebuild_llm_client().await {
                            return Ok(InputResult::Messages(
                                vec![
                                    ChatMessage::System(format!("LLM model set to '{}'.", value)),
                                    ChatMessage::Error(format!(
                                        "Warning: Could not reinitialize LLM client: {}",
                                        e
                                    )),
                                ],
                                None,
                            ));
                        }
                        Ok(InputResult::Messages(
                            vec![ChatMessage::System(format!(
                                "LLM model set to '{}'.",
                                value
                            ))],
                            None,
                        ))
                    }
                    Err(e) => Ok(InputResult::Messages(
                        vec![ChatMessage::Error(e.to_string())],
                        None,
                    )),
                }
            }
        }
    }

    /// Handles natural language input by sending it to the LLM.
    async fn handle_natural_language(&mut self, input: &str) -> Result<InputResult> {
        let tool_context = ToolContext {
            state_db: self.state_db.as_ref(),
            current_connection: self.current_connection_name.as_deref(),
        };

        let result = self
            .llm_service
            .process_query(input, &self.schema, &mut self.conversation, &tool_context)
            .await?;

        self.handle_llm_result(result).await
    }

    /// Converts an LlmResult into an InputResult, executing SQL if present.
    async fn handle_llm_result(&mut self, result: LlmResult) -> Result<InputResult> {
        match result {
            LlmResult::Sql { sql, explanation } => {
                let mut result_messages = Vec::new();

                if let Some(text) = explanation {
                    result_messages.push(ChatMessage::Assistant(text));
                }

                match self
                    .handle_sql_with_source(&sql, QuerySource::Generated)
                    .await?
                {
                    InputResult::Messages(msgs, log_entry) => {
                        result_messages.extend(msgs);
                        Ok(InputResult::Messages(result_messages, log_entry))
                    }
                    InputResult::NeedsConfirmation {
                        sql,
                        classification,
                    } => Ok(InputResult::NeedsConfirmation {
                        sql,
                        classification,
                    }),
                    _ => Ok(InputResult::Messages(result_messages, None)),
                }
            }
            LlmResult::Explanation(text) => Ok(InputResult::Messages(
                vec![ChatMessage::Assistant(text)],
                None,
            )),
        }
    }

    /// Handles SQL execution with safety classification.
    async fn handle_sql(&mut self, sql: &str) -> Result<InputResult> {
        self.handle_sql_with_source(sql, QuerySource::Manual).await
    }

    /// Handles SQL execution with safety classification and a specific source.
    async fn handle_sql_with_source(
        &mut self,
        sql: &str,
        source: QuerySource,
    ) -> Result<InputResult> {
        // Classify the SQL
        let classification = classify_sql(sql);

        match classification.level {
            SafetyLevel::Safe => {
                // Auto-execute safe queries
                // If source is Manual (from /sql), keep it Manual; otherwise mark as Auto
                let effective_source = if source == QuerySource::Manual {
                    QuerySource::Manual
                } else {
                    QuerySource::Auto
                };
                let (messages, log_entry) = self
                    .execute_and_format_with_source(sql, effective_source)
                    .await;
                Ok(InputResult::Messages(messages, log_entry))
            }
            SafetyLevel::Mutating | SafetyLevel::Destructive => {
                // Needs confirmation
                Ok(InputResult::NeedsConfirmation {
                    sql: sql.to_string(),
                    classification,
                })
            }
        }
    }

    /// Executes a SQL query and returns formatted messages with a log entry.
    #[allow(dead_code)]
    pub async fn execute_and_format(
        &mut self,
        sql: &str,
    ) -> (Vec<ChatMessage>, Option<QueryLogEntry>) {
        self.execute_and_format_with_source(sql, QuerySource::Manual)
            .await
    }

    /// Executes a SQL query with a specific source and returns formatted messages with a log entry.
    pub async fn execute_and_format_with_source(
        &mut self,
        sql: &str,
        source: QuerySource,
    ) -> (Vec<ChatMessage>, Option<QueryLogEntry>) {
        let (result, entry) = self.execute_query_with_source(sql, source).await;
        match result {
            Ok(query_result) => {
                let messages = vec![
                    ChatMessage::System(format!("Query executed in {:?}", entry.execution_time)),
                    ChatMessage::Result(query_result),
                ];
                (messages, Some(entry))
            }
            Err(e) => (
                vec![ChatMessage::Error(format!(
                    "Error executing query:\n  {}",
                    e
                ))],
                Some(entry), // Always return the log entry, even for errors
            ),
        }
    }

    /// Executes a SQL query and returns the result with a log entry.
    /// Always returns a log entry, even on error.
    pub async fn execute_query_with_source(
        &mut self,
        sql: &str,
        source: QuerySource,
    ) -> (Result<QueryResult>, QueryLogEntry) {
        let db = match self.db.as_ref() {
            Some(db) => db,
            None => {
                let entry = QueryLogEntry::error_with_source(
                    sql.to_string(),
                    std::time::Duration::ZERO,
                    "No database connection available".to_string(),
                    source,
                );
                return (
                    Err(GlanceError::connection("No database connection available")),
                    entry,
                );
            }
        };

        let start = Instant::now();
        let result = db.execute_query(sql).await;
        let execution_time = start.elapsed();

        self.last_executed_sql = Some(sql.to_string());

        let (status, row_count, error_msg) = match &result {
            Ok(qr) => (QueryStatus::Success, Some(qr.row_count as i64), None),
            Err(e) => (QueryStatus::Error, None, Some(e.to_string())),
        };

        // Record to history only if we have a connection name (skip for unsaved connections)
        if let (Some(state_db), Some(conn_name)) = (&self.state_db, &self.current_connection_name) {
            // Map QuerySource to SubmittedBy
            let submitted_by = match source {
                QuerySource::Manual => SubmittedBy::User,
                QuerySource::Generated | QuerySource::Auto => SubmittedBy::Llm,
            };
            let _ = persistence::history::record_query(
                state_db.pool(),
                conn_name,
                submitted_by,
                sql,
                status,
                Some(execution_time.as_millis() as i64),
                row_count,
                error_msg.as_deref(),
                None,
            )
            .await;
        }

        let entry = match &result {
            Ok(query_result) => QueryLogEntry::success_with_source(
                sql.to_string(),
                execution_time,
                query_result.row_count,
                source,
            ),
            Err(e) => QueryLogEntry::error_with_source(
                sql.to_string(),
                execution_time,
                e.to_string(),
                source,
            ),
        };

        (result.map_err(|e| GlanceError::query(e.to_string())), entry)
    }

    /// Confirms and executes a pending query (user-confirmed LLM-generated query).
    pub async fn confirm_query(&mut self, sql: &str) -> (Vec<ChatMessage>, Option<QueryLogEntry>) {
        self.execute_and_format_with_source(sql, QuerySource::Generated)
            .await
    }

    /// Cancels a pending query and records it in history.
    pub async fn cancel_query(&mut self, sql: Option<&str>) -> ChatMessage {
        // Record the cancellation in history if we have SQL and a connection
        if let (Some(sql), Some(state_db), Some(conn_name)) =
            (sql, &self.state_db, &self.current_connection_name)
        {
            let _ = persistence::history::record_query(
                state_db.pool(),
                conn_name,
                SubmittedBy::Llm, // Cancelled queries are typically LLM-generated (from confirmation dialog)
                sql,
                QueryStatus::Cancelled,
                None, // No execution time
                None, // No row count
                None, // No error message
                None, // No saved_query_id
            )
            .await;
        }
        ChatMessage::System("Query cancelled.".to_string())
    }

    /// Returns the current connection name.
    #[allow(dead_code)]
    pub fn current_connection(&self) -> Option<&str> {
        self.current_connection_name.as_deref()
    }

    /// Returns the secret storage status.
    #[allow(dead_code)]
    pub fn secret_storage_status(&self) -> Option<SecretStorageStatus> {
        self.state_db.as_ref().map(|db| db.secret_storage_status())
    }

    /// Handles /connect <name> command - switch to a saved connection.
    async fn handle_connect(&mut self, args: &str) -> Result<InputResult> {
        if args.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error("Usage: /connect <name>".to_string())],
                None,
            ));
        }

        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(
                        "State database not available.".to_string(),
                    )],
                    None,
                ));
            }
        };

        let profile = persistence::connections::get_connection(state_db.pool(), args).await?;
        let profile = match profile {
            Some(p) => p,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(format!(
                        "Connection '{}' not found.",
                        args
                    ))],
                    None,
                ));
            }
        };

        let password = persistence::connections::get_connection_password(
            state_db.pool(),
            args,
            state_db.secrets(),
        )
        .await?;

        let config = ConnectionConfig {
            backend: profile.backend,
            host: profile.host.clone(),
            port: profile.port,
            database: Some(profile.database.clone()),
            user: profile.username.clone(),
            password: password.clone(),
            sslmode: profile.sslmode.clone(),
            extras: profile.extras.clone(),
        };

        tracing::debug!(
            "Connecting with: host={:?}, port={}, db={:?}, user={:?}, has_password={}",
            config.host,
            config.port,
            config.database,
            config.user,
            config.password.is_some()
        );

        let db = match crate::db::connect(&config).await {
            Ok(db) => db,
            Err(e) => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(format!("Failed to connect: {}", e))],
                    None,
                ));
            }
        };

        let schema = db.introspect_schema().await?;

        if let Some(old_db) = self.db.take() {
            let _ = old_db.close().await;
        }

        self.db = Some(db);
        self.schema = schema;
        self.conversation.clear();
        self.current_connection_name = Some(args.to_string());
        self.last_executed_sql = None;

        persistence::connections::touch_connection(state_db.pool(), args).await?;

        Ok(InputResult::ConnectionSwitch {
            messages: vec![ChatMessage::System(format!(
                "Connected to {} ({})",
                args, profile.database
            ))],
            connection_info: format!("{} ({})", args, profile.database),
            schema: self.schema.clone(),
        })
    }

    /// Closes the database connection and cleans up resources.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(db) = self.db.take() {
            db.close().await?;
        }
        if let Some(state_db) = self.state_db.take() {
            if let Ok(db) = Arc::try_unwrap(state_db) {
                db.close().await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Column, ForeignKey, Table};

    fn sample_schema() -> Schema {
        Schema {
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column::new("id", "integer").nullable(false),
                        Column::new("email", "varchar(255)").nullable(false),
                        Column::new("name", "varchar(100)"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column::new("id", "integer").nullable(false),
                        Column::new("user_id", "integer").nullable(false),
                        Column::new("total", "numeric(10,2)").nullable(false),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
            ],
            foreign_keys: vec![ForeignKey::new(
                "orders",
                vec!["user_id".to_string()],
                "users",
                vec!["id".to_string()],
            )],
        }
    }

    #[tokio::test]
    async fn test_handle_empty_input() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let result = orchestrator.handle_input("").await.unwrap();
        assert!(matches!(result, InputResult::None));
    }

    #[tokio::test]
    async fn test_handle_whitespace_input() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let result = orchestrator.handle_input("   \n\t  ").await.unwrap();
        assert!(matches!(result, InputResult::None));
    }

    #[tokio::test]
    async fn test_handle_help_command() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let result = orchestrator.handle_input("/help").await.unwrap();

        match result {
            InputResult::Messages(msgs, log_entry) => {
                assert_eq!(msgs.len(), 1);
                assert!(log_entry.is_none());
                match &msgs[0] {
                    ChatMessage::System(text) => {
                        assert!(text.contains("/sql"));
                        assert!(text.contains("/clear"));
                        assert!(text.contains("/schema"));
                    }
                    _ => panic!("Expected System message"),
                }
            }
            _ => panic!("Expected Messages result"),
        }
    }

    #[tokio::test]
    async fn test_handle_quit_command() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());

        let result = orchestrator.handle_input("/quit").await.unwrap();
        assert!(matches!(result, InputResult::Exit));

        let result = orchestrator.handle_input("/exit").await.unwrap();
        assert!(matches!(result, InputResult::Exit));
    }

    #[tokio::test]
    async fn test_handle_clear_command() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());

        // Add some conversation history
        orchestrator.conversation.add_user("test");
        orchestrator.conversation.add_assistant("response");
        assert!(!orchestrator.conversation.is_empty());

        let result = orchestrator.handle_input("/clear").await.unwrap();

        match result {
            InputResult::Messages(msgs, log_entry) => {
                assert_eq!(msgs.len(), 1);
                assert!(log_entry.is_none());
                match &msgs[0] {
                    ChatMessage::System(text) => {
                        assert!(text.contains("cleared"));
                    }
                    _ => panic!("Expected System message"),
                }
            }
            _ => panic!("Expected Messages result"),
        }

        assert!(orchestrator.conversation.is_empty());
    }

    #[tokio::test]
    async fn test_handle_schema_command() {
        let schema = sample_schema();
        let mut orchestrator = Orchestrator::with_mock_llm(None, schema);

        let result = orchestrator.handle_input("/schema").await.unwrap();

        match result {
            InputResult::Messages(msgs, log_entry) => {
                assert_eq!(msgs.len(), 1);
                assert!(log_entry.is_none());
                match &msgs[0] {
                    ChatMessage::System(text) => {
                        assert!(text.contains("users"));
                        assert!(text.contains("orders"));
                    }
                    _ => panic!("Expected System message"),
                }
            }
            _ => panic!("Expected Messages result"),
        }
    }

    #[tokio::test]
    async fn test_handle_unknown_command() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let result = orchestrator.handle_input("/unknown").await.unwrap();

        match result {
            InputResult::Messages(msgs, log_entry) => {
                assert_eq!(msgs.len(), 1);
                assert!(log_entry.is_none());
                match &msgs[0] {
                    ChatMessage::Error(text) => {
                        assert!(text.contains("Unknown command"));
                        assert!(text.contains("/unknown"));
                    }
                    _ => panic!("Expected Error message"),
                }
            }
            _ => panic!("Expected Messages result"),
        }
    }

    #[tokio::test]
    async fn test_handle_sql_command_empty() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let result = orchestrator.handle_input("/sql").await.unwrap();

        match result {
            InputResult::Messages(msgs, log_entry) => {
                assert_eq!(msgs.len(), 1);
                assert!(log_entry.is_none());
                match &msgs[0] {
                    ChatMessage::Error(text) => {
                        assert!(text.contains("Usage"));
                    }
                    _ => panic!("Expected Error message"),
                }
            }
            _ => panic!("Expected Messages result"),
        }
    }

    #[tokio::test]
    async fn test_sql_classification_safe() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());

        // Safe query should not need confirmation (but will fail without DB)
        let result = orchestrator.handle_input("/sql SELECT 1").await.unwrap();

        // Without a database, this will return an error message
        match result {
            InputResult::Messages(msgs, _log_entry) => {
                // Should have error about no database connection
                assert!(!msgs.is_empty());
            }
            _ => panic!("Expected Messages result"),
        }
    }

    #[tokio::test]
    async fn test_sql_classification_mutating() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());

        let result = orchestrator
            .handle_input("/sql INSERT INTO users (name) VALUES ('test')")
            .await
            .unwrap();

        match result {
            InputResult::NeedsConfirmation {
                sql,
                classification,
            } => {
                assert!(sql.contains("INSERT"));
                assert_eq!(classification.level, SafetyLevel::Mutating);
            }
            _ => panic!("Expected NeedsConfirmation result"),
        }
    }

    #[tokio::test]
    async fn test_sql_classification_destructive() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());

        let result = orchestrator
            .handle_input("/sql DELETE FROM users")
            .await
            .unwrap();

        match result {
            InputResult::NeedsConfirmation {
                sql,
                classification,
            } => {
                assert!(sql.contains("DELETE"));
                assert_eq!(classification.level, SafetyLevel::Destructive);
                assert!(classification.requires_warning());
            }
            _ => panic!("Expected NeedsConfirmation result"),
        }
    }

    #[tokio::test]
    async fn test_cancel_query() {
        let mut orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let msg = orchestrator.cancel_query(None).await;

        match msg {
            ChatMessage::System(text) => {
                assert!(text.contains("cancelled"));
            }
            _ => panic!("Expected System message"),
        }
    }
}
