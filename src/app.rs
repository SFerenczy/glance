//! Core orchestrator for Glance.
//!
//! Coordinates the database client, LLM client, safety classifier,
//! and application state to implement the main chat loop.

use std::time::Instant;

use crate::config::ConnectionConfig;
use crate::db::{DatabaseClient, PostgresClient, QueryResult, Schema};
use crate::error::{GlanceError, Result};
use crate::llm::{
    build_messages, parse_llm_response, Conversation, LlmClient, LlmProvider, MockLlmClient,
};
use crate::safety::{classify_sql, ClassificationResult, SafetyLevel};
use crate::tui::app::{ChatMessage, QueryLogEntry};

/// Help text displayed for the /help command.
const HELP_TEXT: &str = r#"Available commands:
  /sql <query>  - Execute raw SQL directly
  /clear        - Clear chat history and LLM context
  /schema       - Display database schema
  /help         - Show this help message
  /quit, /exit  - Exit the application

Keyboard shortcuts:
  Ctrl+C, Ctrl+Q  - Exit application
  Tab             - Switch focus between panels
  Enter           - Submit input
  Esc             - Cancel/close modal
  ↑/↓             - Scroll or navigate
  Page Up/Down    - Scroll by page"#;

/// Result of processing user input.
#[derive(Debug)]
pub enum InputResult {
    /// No action needed (empty input, etc.)
    None,
    /// Messages to add to the chat.
    Messages(Vec<ChatMessage>),
    /// A query needs confirmation before execution.
    NeedsConfirmation {
        sql: String,
        classification: ClassificationResult,
    },
    /// Application should exit.
    Exit,
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

        Ok(Self {
            db: Some(Box::new(db)),
            llm,
            schema,
            conversation: Conversation::new(),
        })
    }

    /// Creates an orchestrator with a mock LLM for testing.
    #[allow(dead_code)]
    pub fn with_mock_llm(db: Option<Box<dyn DatabaseClient>>, schema: Schema) -> Self {
        Self {
            db,
            llm: Box::new(MockLlmClient::new()),
            schema,
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
                    return Ok(InputResult::Messages(vec![ChatMessage::Error(
                        "Usage: /sql <query>".to_string(),
                    )]));
                }
                self.handle_sql(args).await
            }
            "/clear" => {
                self.conversation.clear();
                Ok(InputResult::Messages(vec![ChatMessage::System(
                    "Chat history and context cleared.".to_string(),
                )]))
            }
            "/schema" => {
                let schema_text = self.schema.format_for_display();
                Ok(InputResult::Messages(vec![ChatMessage::System(
                    schema_text,
                )]))
            }
            "/quit" | "/exit" => Ok(InputResult::Exit),
            "/help" => Ok(InputResult::Messages(vec![ChatMessage::System(
                HELP_TEXT.to_string(),
            )])),
            _ => Ok(InputResult::Messages(vec![ChatMessage::Error(format!(
                "Unknown command: {}. Type /help for available commands.",
                command
            ))])),
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

        // Add any explanatory text
        if !parsed.text.is_empty() {
            result_messages.push(ChatMessage::Assistant(parsed.text));
        }

        // If SQL was found, handle it
        if let Some(sql) = parsed.sql {
            match self.handle_sql(&sql).await? {
                InputResult::Messages(msgs) => result_messages.extend(msgs),
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

        Ok(InputResult::Messages(result_messages))
    }

    /// Handles SQL execution with safety classification.
    async fn handle_sql(&mut self, sql: &str) -> Result<InputResult> {
        // Classify the SQL
        let classification = classify_sql(sql);

        match classification.level {
            SafetyLevel::Safe => {
                // Auto-execute safe queries
                let messages = self.execute_and_format(sql).await;
                Ok(InputResult::Messages(messages))
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

    /// Executes a SQL query and returns formatted messages.
    pub async fn execute_and_format(&mut self, sql: &str) -> Vec<ChatMessage> {
        match self.execute_query(sql).await {
            Ok((result, entry)) => {
                vec![
                    ChatMessage::System(format!("Query executed in {:?}", entry.execution_time)),
                    ChatMessage::Result(result),
                ]
            }
            Err(e) => {
                vec![ChatMessage::Error(format!(
                    "Error executing query:\n  {}",
                    e
                ))]
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
    pub async fn confirm_query(&mut self, sql: &str) -> Vec<ChatMessage> {
        self.execute_and_format(sql).await
    }

    /// Cancels a pending query.
    pub fn cancel_query(&self) -> ChatMessage {
        ChatMessage::System("Query cancelled.".to_string())
    }

    /// Closes the database connection and cleans up resources.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(db) = self.db.take() {
            db.close().await?;
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
            InputResult::Messages(msgs) => {
                assert_eq!(msgs.len(), 1);
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
            InputResult::Messages(msgs) => {
                assert_eq!(msgs.len(), 1);
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
            InputResult::Messages(msgs) => {
                assert_eq!(msgs.len(), 1);
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
            InputResult::Messages(msgs) => {
                assert_eq!(msgs.len(), 1);
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
            InputResult::Messages(msgs) => {
                assert_eq!(msgs.len(), 1);
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
            InputResult::Messages(msgs) => {
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
