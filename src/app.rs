//! Core orchestrator for Glance.
//!
//! Coordinates the database client, LLM client, safety classifier,
//! and application state to implement the main chat loop.

use std::sync::Arc;
use std::time::Instant;

use crate::config::ConnectionConfig;
use crate::db::{DatabaseClient, PostgresClient, QueryResult, Schema};
use crate::error::{GlanceError, Result};
use crate::llm::{
    build_messages, parse_llm_response, Conversation, LlmClient, LlmProvider, MockLlmClient,
};
use crate::persistence::{
    self, ConnectionProfile, HistoryEntry, HistoryFilter, QueryStatus, SavedQuery,
    SavedQueryFilter, SecretStorageStatus, StateDb, SubmittedBy,
};
use crate::safety::{classify_sql, ClassificationResult, SafetyLevel};
use crate::tui::app::{ChatMessage, QueryLogEntry};

/// Help text displayed for the /help command.
const HELP_TEXT: &str = r#"Available commands:
  /sql <query>     - Execute raw SQL directly
  /clear           - Clear chat history and LLM context
  /schema          - Display database schema
  /vim             - Toggle vim-style navigation mode
  /help            - Show this help message
  /quit, /exit     - Exit the application

Connection commands:
  /connections     - List saved connections
  /connect <name>  - Switch to a saved connection
  /conn add <name> host=... database=... [--test]
  /conn edit <name> - Edit an existing connection
  /conn delete <name> - Delete a connection

History commands:
  /history [--conn <name>] [--text <filter>] [--limit N]
  /history clear   - Clear query history

Saved queries:
  /savequery <name> [#tags...] - Save current/last query
  /queries [--tag <tag>] [--text <filter>]
  /usequery <name> - Load a saved query
  /query delete <name> - Delete a saved query

LLM settings:
  /llm provider <openai|anthropic|ollama>
  /llm model <name>
  /llm key         - Set API key (masked input)

Keyboard shortcuts:
  Ctrl+C, Ctrl+Q  - Exit application
  Tab             - Switch focus between panels
  Enter           - Submit input
  Esc             - Clear input (or exit to Normal mode in vim mode)
  ↑/↓             - History navigation or scroll
  Page Up/Down    - Scroll by page"#;

/// Result of processing user input.
#[derive(Debug)]
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
}

/// The main orchestrator that coordinates all components.
pub struct Orchestrator {
    /// Database client for executing queries.
    db: Option<Box<dyn DatabaseClient>>,
    /// LLM client for generating SQL from natural language.
    llm: Box<dyn LlmClient>,
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
            llm,
            schema,
            conversation: Conversation::new(),
            state_db: None,
            current_connection_name: None,
            last_executed_sql: None,
        }
    }

    /// Creates an orchestrator by connecting to the database and initializing components.
    pub async fn connect(connection: &ConnectionConfig, llm_provider: LlmProvider) -> Result<Self> {
        // Connect to database
        let db = PostgresClient::connect(connection).await?;

        // Introspect schema
        let schema = db.introspect_schema().await?;

        // Create LLM client based on provider
        let llm: Box<dyn LlmClient> = match llm_provider {
            LlmProvider::OpenAi => {
                use crate::llm::OpenAiClient;
                Box::new(OpenAiClient::from_env()?)
            }
            LlmProvider::Anthropic => {
                use crate::llm::AnthropicClient;
                Box::new(AnthropicClient::from_env()?)
            }
            LlmProvider::Ollama => {
                use crate::llm::OllamaClient;
                Box::new(OllamaClient::from_env()?)
            }
            LlmProvider::Mock => Box::new(MockLlmClient::new()),
        };

        // Open state database
        let state_db = StateDb::open_default().await.ok().map(Arc::new);

        Ok(Self {
            db: Some(Box::new(db)),
            llm,
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
            llm: Box::new(MockLlmClient::new()),
            schema,
            state_db: None,
            current_connection_name: None,
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
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command.as_str() {
            "/sql" => {
                if args.is_empty() {
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::Error("Usage: /sql <query>".to_string())],
                        None,
                    ));
                }
                self.handle_sql(args).await
            }
            "/clear" => {
                self.conversation.clear();
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(
                        "Chat history and context cleared.".to_string(),
                    )],
                    None,
                ))
            }
            "/schema" => {
                let schema_text = self.schema.format_for_display();
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(schema_text)],
                    None,
                ))
            }
            "/quit" | "/exit" => Ok(InputResult::Exit),
            "/vim" => Ok(InputResult::ToggleVimMode),
            "/help" => Ok(InputResult::Messages(
                vec![ChatMessage::System(HELP_TEXT.to_string())],
                None,
            )),
            "/connections" => self.handle_connections_list().await,
            "/connect" => self.handle_connect(args).await,
            "/conn" => self.handle_conn_command(args).await,
            "/history" => self.handle_history_command(args).await,
            "/savequery" => self.handle_savequery(args).await,
            "/queries" => self.handle_queries_list(args).await,
            "/usequery" => self.handle_usequery(args).await,
            "/query" => self.handle_query_command(args).await,
            "/llm" => self.handle_llm_command(args).await,
            _ => Ok(InputResult::Messages(
                vec![ChatMessage::Error(format!(
                    "Unknown command: {}. Type /help for available commands.",
                    command
                ))],
                None,
            )),
        }
    }

    /// Handles natural language input by sending it to the LLM.
    async fn handle_natural_language(&mut self, input: &str) -> Result<InputResult> {
        // Add user message to conversation
        self.conversation.add_user(input);

        // Build messages for LLM
        let messages = build_messages(&self.schema, &self.conversation);

        // Get LLM response
        let response = self.llm.complete(&messages).await?;

        // Add assistant response to conversation
        self.conversation.add_assistant(&response);

        // Parse the response to extract SQL
        let parsed = parse_llm_response(&response);

        let mut result_messages = Vec::new();
        let mut log_entry = None;

        // Add any explanatory text
        if !parsed.text.is_empty() {
            result_messages.push(ChatMessage::Assistant(parsed.text));
        }

        // If SQL was found, handle it
        if let Some(sql) = parsed.sql {
            match self.handle_sql(&sql).await? {
                InputResult::Messages(msgs, entry) => {
                    result_messages.extend(msgs);
                    log_entry = entry;
                }
                InputResult::NeedsConfirmation {
                    sql,
                    classification,
                } => {
                    return Ok(InputResult::NeedsConfirmation {
                        sql,
                        classification,
                    });
                }
                _ => {}
            }
        }

        Ok(InputResult::Messages(result_messages, log_entry))
    }

    /// Handles SQL execution with safety classification.
    async fn handle_sql(&mut self, sql: &str) -> Result<InputResult> {
        // Classify the SQL
        let classification = classify_sql(sql);

        match classification.level {
            SafetyLevel::Safe => {
                // Auto-execute safe queries
                let (messages, log_entry) = self.execute_and_format(sql).await;
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
    pub async fn execute_and_format(&mut self, sql: &str) -> (Vec<ChatMessage>, Option<QueryLogEntry>) {
        match self.execute_query(sql).await {
            Ok((result, entry)) => {
                let messages = vec![
                    ChatMessage::System(format!("Query executed in {:?}", entry.execution_time)),
                    ChatMessage::Result(result),
                ];
                (messages, Some(entry))
            }
            Err(e) => {
                (
                    vec![ChatMessage::Error(format!(
                        "Error executing query:\n  {}",
                        e
                    ))],
                    None,
                )
            }
        }
    }

    /// Executes a SQL query and returns the result with a log entry.
    pub async fn execute_query(&mut self, sql: &str) -> Result<(QueryResult, QueryLogEntry)> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| GlanceError::connection("No database connection available"))?;

        let start = Instant::now();
        let result = db.execute_query(sql).await;
        let execution_time = start.elapsed();

        self.last_executed_sql = Some(sql.to_string());

        let (status, row_count, error_msg) = match &result {
            Ok(qr) => (QueryStatus::Success, Some(qr.row_count as i64), None),
            Err(e) => (QueryStatus::Error, None, Some(e.to_string())),
        };

        if let Some(state_db) = &self.state_db {
            let conn_name = self.current_connection_name.as_deref().unwrap_or("default");
            let _ = persistence::history::record_query(
                state_db.pool(),
                conn_name,
                SubmittedBy::User,
                sql,
                status,
                Some(execution_time.as_millis() as i64),
                row_count,
                error_msg.as_deref(),
                None,
            )
            .await;
        }

        match result {
            Ok(query_result) => {
                let row_count = query_result.row_count;
                let entry = QueryLogEntry::success(sql.to_string(), execution_time, row_count);
                Ok((query_result, entry))
            }
            Err(e) => {
                let _entry = QueryLogEntry::error(sql.to_string(), execution_time, e.to_string());
                Err(GlanceError::query(format!(
                    "{}\n\nQuery log entry created with error status.",
                    e
                )))
            }
        }
    }

    /// Confirms and executes a pending query.
    pub async fn confirm_query(&mut self, sql: &str) -> (Vec<ChatMessage>, Option<QueryLogEntry>) {
        self.execute_and_format(sql).await
    }

    /// Cancels a pending query.
    pub fn cancel_query(&self) -> ChatMessage {
        ChatMessage::System("Query cancelled.".to_string())
    }

    /// Returns the current connection name.
    pub fn current_connection(&self) -> Option<&str> {
        self.current_connection_name.as_deref()
    }

    /// Returns the secret storage status.
    pub fn secret_storage_status(&self) -> Option<SecretStorageStatus> {
        self.state_db.as_ref().map(|db| db.secret_storage_status())
    }

    /// Handles /connections command - list saved connections.
    async fn handle_connections_list(&self) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => db,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let connections = persistence::connections::list_connections(state_db.pool()).await?;

        if connections.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::System(
                    "No saved connections. Use /conn add <name> to add one.".to_string(),
                )],
                None,
            ));
        }

        let mut output = String::from("Saved connections:\n");
        for conn in &connections {
            let last_used = conn
                .last_used_at
                .as_deref()
                .unwrap_or("never");
            output.push_str(&format!(
                "  • {} - {} @ {}:{} (last used: {})\n",
                conn.name,
                conn.database,
                conn.redacted_host(),
                conn.port,
                last_used
            ));
        }

        Ok(InputResult::Messages(
            vec![ChatMessage::System(output.trim_end().to_string())],
            None,
        ))
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
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let profile = persistence::connections::get_connection(state_db.pool(), args).await?;
        let profile = match profile {
            Some(p) => p,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error(format!("Connection '{}' not found.", args))],
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
            host: profile.host.clone(),
            port: profile.port,
            database: Some(profile.database.clone()),
            user: profile.username.clone(),
            password,
            sslmode: profile.sslmode.clone(),
            extras: profile.extras.clone(),
        };

        let db = match PostgresClient::connect(&config).await {
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

        self.db = Some(Box::new(db));
        self.schema = schema;
        self.conversation.clear();
        self.current_connection_name = Some(args.to_string());
        self.last_executed_sql = None;

        persistence::connections::touch_connection(state_db.pool(), args).await?;

        Ok(InputResult::Messages(
            vec![ChatMessage::System(format!(
                "Connected to {} ({})",
                args,
                profile.database
            ))],
            None,
        ))
    }

    /// Handles /conn add|edit|delete commands.
    async fn handle_conn_command(&mut self, args: &str) -> Result<InputResult> {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let name = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match subcommand.as_str() {
            "add" => {
                if name.is_empty() {
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::Error("Usage: /conn add <name>".to_string())],
                        None,
                    ));
                }
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(format!(
                        "To add connection '{}', provide details in format:\n\
                         /conn add {} host=<host> port=<port> database=<db> user=<user> [password=<pwd>] [sslmode=<mode>]\n\n\
                         Example: /conn add {} host=localhost port=5432 database=mydb user=postgres",
                        name, name, name
                    ))],
                    None,
                ))
            }
            "edit" => {
                if name.is_empty() {
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::Error("Usage: /conn edit <name>".to_string())],
                        None,
                    ));
                }
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(format!(
                        "To edit connection '{}', use:\n\
                         /conn edit {} <field>=<value> ...\n\n\
                         Fields: host, port, database, user, password, sslmode",
                        name, name
                    ))],
                    None,
                ))
            }
            "delete" => {
                if name.is_empty() {
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::Error("Usage: /conn delete <name>".to_string())],
                        None,
                    ));
                }

                let state_db = match &self.state_db {
                    Some(db) => Arc::clone(db),
                    None => {
                        return Ok(InputResult::Messages(
                            vec![ChatMessage::Error("State database not available.".to_string())],
                            None,
                        ));
                    }
                };

                match persistence::connections::delete_connection(
                    state_db.pool(),
                    name,
                    state_db.secrets(),
                )
                .await
                {
                    Ok(()) => Ok(InputResult::Messages(
                        vec![ChatMessage::System(format!("Connection '{}' deleted.", name))],
                        None,
                    )),
                    Err(e) => Ok(InputResult::Messages(
                        vec![ChatMessage::Error(e.to_string())],
                        None,
                    )),
                }
            }
            _ if !subcommand.is_empty() && subcommand.contains('=') => {
                self.handle_conn_add_with_params(args).await
            }
            _ => Ok(InputResult::Messages(
                vec![ChatMessage::Error(
                    "Usage: /conn add|edit|delete <name>".to_string(),
                )],
                None,
            )),
        }
    }

    /// Handles /conn add <name> with inline parameters.
    async fn handle_conn_add_with_params(&mut self, args: &str) -> Result<InputResult> {
        let mut name = String::new();
        let mut host = None;
        let mut port = 5432u16;
        let mut database = None;
        let mut user = None;
        let mut password = None;
        let mut sslmode = None;
        let mut test_connection = false;

        for part in args.split_whitespace() {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "host" => host = Some(value.to_string()),
                    "port" => port = value.parse().unwrap_or(5432),
                    "database" | "db" => database = Some(value.to_string()),
                    "user" => user = Some(value.to_string()),
                    "password" | "pwd" => password = Some(value.to_string()),
                    "sslmode" => sslmode = Some(value.to_string()),
                    _ => {}
                }
            } else if part == "--test" || part == "-t" {
                test_connection = true;
            } else if name.is_empty() {
                name = part.to_string();
            }
        }

        if name.is_empty() || database.is_none() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error(
                    "Connection name and database are required.".to_string(),
                )],
                None,
            ));
        }

        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let db_name = database.clone().unwrap();

        if test_connection {
            let test_config = ConnectionConfig {
                host: host.clone(),
                port,
                database: database.clone(),
                user: user.clone(),
                password: password.clone(),
                sslmode: sslmode.clone(),
                extras: None,
            };

            match PostgresClient::connect(&test_config).await {
                Ok(db) => {
                    let _ = db.close().await;
                }
                Err(e) => {
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::Error(format!(
                            "Connection test failed: {}. Connection not saved.",
                            e
                        ))],
                        None,
                    ));
                }
            }
        }

        let profile = ConnectionProfile {
            name: name.clone(),
            database: db_name,
            host,
            port,
            username: user,
            sslmode,
            extras: None,
            password_storage: persistence::connections::PasswordStorage::None,
            created_at: String::new(),
            updated_at: String::new(),
            last_used_at: None,
        };

        match persistence::connections::create_connection(
            state_db.pool(),
            &profile,
            password.as_deref(),
            state_db.secrets(),
        )
        .await
        {
            Ok(()) => {
                let test_msg = if test_connection {
                    " (connection tested successfully)"
                } else {
                    ""
                };
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(format!(
                        "Connection '{}' saved{}. Use /connect {} to use it.",
                        name, test_msg, name
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

    /// Handles /history command.
    async fn handle_history_command(&self, args: &str) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => db,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        if args.trim() == "clear" {
            let count = persistence::history::clear_history(state_db.pool()).await?;
            return Ok(InputResult::Messages(
                vec![ChatMessage::System(format!("Cleared {} history entries.", count))],
                None,
            ));
        }

        let mut filter = HistoryFilter::default();

        let mut iter = args.split_whitespace().peekable();
        while let Some(arg) = iter.next() {
            match arg {
                "--conn" => {
                    if let Some(val) = iter.next() {
                        filter.connection_name = Some(val.to_string());
                    }
                }
                "--text" => {
                    if let Some(val) = iter.next() {
                        filter.text_search = Some(val.to_string());
                    }
                }
                "--limit" => {
                    if let Some(val) = iter.next() {
                        filter.limit = val.parse().ok();
                    }
                }
                "--since" => {
                    if let Some(val) = iter.next() {
                        filter.since_days = val.parse().ok();
                    }
                }
                _ => {}
            }
        }

        if filter.limit.is_none() {
            filter.limit = Some(20);
        }

        let entries = persistence::history::list_history(state_db.pool(), &filter).await?;

        if entries.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::System("No history entries found.".to_string())],
                None,
            ));
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
                status_icon, entry.created_at, sql_preview.replace('\n', " ")
            ));
        }

        Ok(InputResult::Messages(
            vec![ChatMessage::System(output.trim_end().to_string())],
            None,
        ))
    }

    /// Handles /savequery <name> [#tags...] command.
    async fn handle_savequery(&mut self, args: &str) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let mut name = String::new();
        let mut tags = Vec::new();

        for part in args.split_whitespace() {
            if part.starts_with('#') {
                tags.push(part.trim_start_matches('#').to_string());
            } else if name.is_empty() {
                name = part.to_string();
            }
        }

        if name.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error("Usage: /savequery <name> [#tags...]".to_string())],
                None,
            ));
        }

        let sql = match &self.last_executed_sql {
            Some(sql) => sql.clone(),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("No query to save. Execute a query first.".to_string())],
                    None,
                ));
            }
        };

        match persistence::saved_queries::create_saved_query(
            state_db.pool(),
            &name,
            &sql,
            None,
            self.current_connection_name.as_deref(),
            &tags,
        )
        .await
        {
            Ok(_id) => Ok(InputResult::Messages(
                vec![ChatMessage::System(format!("Query saved as '{}'.", name))],
                None,
            )),
            Err(e) => Ok(InputResult::Messages(
                vec![ChatMessage::Error(e.to_string())],
                None,
            )),
        }
    }

    /// Handles /queries command.
    async fn handle_queries_list(&self, args: &str) -> Result<InputResult> {
        let state_db = match &self.state_db {
            Some(db) => db,
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let mut filter = SavedQueryFilter {
            connection_name: self.current_connection_name.clone(),
            include_global: true,
            ..Default::default()
        };

        let mut iter = args.split_whitespace().peekable();
        while let Some(arg) = iter.next() {
            match arg {
                "--tag" => {
                    if let Some(val) = iter.next() {
                        filter.tag = Some(val.trim_start_matches('#').to_string());
                    }
                }
                "--text" => {
                    if let Some(val) = iter.next() {
                        filter.text_search = Some(val.to_string());
                    }
                }
                "--all" => {
                    filter.connection_name = None;
                }
                "--conn" => {
                    if let Some(val) = iter.next() {
                        filter.connection_name = Some(val.to_string());
                    }
                }
                _ => {}
            }
        }

        let queries = persistence::saved_queries::list_saved_queries(state_db.pool(), &filter).await?;

        if queries.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::System("No saved queries found.".to_string())],
                None,
            ));
        }

        let mut output = String::from("Saved queries:\n");
        for query in &queries {
            let tags_str = if query.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", query.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "))
            };
            let scope = query
                .connection_name
                .as_deref()
                .unwrap_or("global");
            output.push_str(&format!(
                "  • {} ({}){} - used {} times\n",
                query.name, scope, tags_str, query.usage_count
            ));
        }

        Ok(InputResult::Messages(
            vec![ChatMessage::System(output.trim_end().to_string())],
            None,
        ))
    }

    /// Handles /usequery <name> command.
    async fn handle_usequery(&mut self, args: &str) -> Result<InputResult> {
        if args.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error("Usage: /usequery <name>".to_string())],
                None,
            ));
        }

        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        let query = persistence::saved_queries::get_saved_query_by_name(
            state_db.pool(),
            args,
            self.current_connection_name.as_deref(),
        )
        .await?;

        match query {
            Some(q) => {
                persistence::saved_queries::record_usage(state_db.pool(), q.id).await?;
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(format!(
                        "Loaded query '{}'. Use /sql to execute:\n\n/sql {}",
                        q.name, q.sql
                    ))],
                    None,
                ))
            }
            None => Ok(InputResult::Messages(
                vec![ChatMessage::Error(format!("Saved query '{}' not found.", args))],
                None,
            )),
        }
    }

    /// Handles /query delete <name> command.
    async fn handle_query_command(&mut self, args: &str) -> Result<InputResult> {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let name = parts.get(1).map(|s| s.trim()).unwrap_or("");

        if subcommand != "delete" {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error("Usage: /query delete <name>".to_string())],
                None,
            ));
        }

        if name.is_empty() {
            return Ok(InputResult::Messages(
                vec![ChatMessage::Error("Usage: /query delete <name>".to_string())],
                None,
            ));
        }

        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        match persistence::saved_queries::delete_saved_query_by_name(
            state_db.pool(),
            name,
            self.current_connection_name.as_deref(),
        )
        .await
        {
            Ok(()) => Ok(InputResult::Messages(
                vec![ChatMessage::System(format!("Saved query '{}' deleted.", name))],
                None,
            )),
            Err(e) => Ok(InputResult::Messages(
                vec![ChatMessage::Error(e.to_string())],
                None,
            )),
        }
    }

    /// Handles /llm provider|model|key commands.
    async fn handle_llm_command(&mut self, args: &str) -> Result<InputResult> {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let value = parts.get(1).map(|s| s.trim()).unwrap_or("");

        let state_db = match &self.state_db {
            Some(db) => Arc::clone(db),
            None => {
                return Ok(InputResult::Messages(
                    vec![ChatMessage::Error("State database not available.".to_string())],
                    None,
                ));
            }
        };

        match subcommand.as_str() {
            "provider" => {
                if value.is_empty() {
                    let settings = persistence::llm_settings::get_llm_settings(state_db.pool()).await?;
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::System(format!(
                            "Current provider: {}. Use /llm provider <openai|anthropic|ollama> to change.",
                            settings.provider
                        ))],
                        None,
                    ));
                }

                match persistence::llm_settings::set_provider(state_db.pool(), value).await {
                    Ok(()) => {
                        self.conversation.clear();
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
            "model" => {
                if value.is_empty() {
                    let settings = persistence::llm_settings::get_llm_settings(state_db.pool()).await?;
                    return Ok(InputResult::Messages(
                        vec![ChatMessage::System(format!(
                            "Current model: {}. Use /llm model <name> to change.",
                            settings.model
                        ))],
                        None,
                    ));
                }

                match persistence::llm_settings::set_model(state_db.pool(), value).await {
                    Ok(()) => Ok(InputResult::Messages(
                        vec![ChatMessage::System(format!("LLM model set to '{}'.", value))],
                        None,
                    )),
                    Err(e) => Ok(InputResult::Messages(
                        vec![ChatMessage::Error(e.to_string())],
                        None,
                    )),
                }
            }
            "key" => {
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(
                        "API key input not yet implemented. Set via environment variable for now.".to_string(),
                    )],
                    None,
                ))
            }
            _ => {
                let settings = persistence::llm_settings::get_llm_settings(state_db.pool()).await?;
                Ok(InputResult::Messages(
                    vec![ChatMessage::System(format!(
                        "LLM settings:\n  Provider: {}\n  Model: {}\n\nCommands:\n  /llm provider <name>\n  /llm model <name>\n  /llm key",
                        settings.provider, settings.model
                    ))],
                    None,
                ))
            }
        }
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
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let msg = orchestrator.cancel_query();

        match msg {
            ChatMessage::System(text) => {
                assert!(text.contains("cancelled"));
            }
            _ => panic!("Expected System message"),
        }
    }
}
