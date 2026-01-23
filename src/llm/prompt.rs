//! Prompt construction for LLM requests.
//!
//! Builds system prompts with database schema context.
//!
//! # Privacy
//!
//! Connection context is redacted before being sent to the LLM:
//! - Passwords are never included
//! - Hostnames and usernames are never included
//! - Only connection label and database name are exposed

use crate::db::Schema;
use crate::llm::types::{Conversation, Message};
use std::sync::Arc;

/// Redacted connection context safe for LLM consumption.
///
/// Contains only non-sensitive information about the current connection.
/// Passwords, hostnames, and usernames are explicitly excluded.
#[derive(Debug, Clone, Default)]
pub struct ConnectionContext {
    /// User-provided connection label (e.g., "production", "staging").
    pub label: Option<String>,
    /// Database name.
    pub database: Option<String>,
}

impl ConnectionContext {
    /// Creates a new connection context.
    pub fn new(label: Option<String>, database: Option<String>) -> Self {
        Self { label, database }
    }

    /// Formats the connection context for inclusion in the system prompt.
    fn format_for_prompt(&self) -> Option<String> {
        match (&self.label, &self.database) {
            (Some(label), Some(db)) => Some(format!("Connection: {} (database: {})", label, db)),
            (Some(label), None) => Some(format!("Connection: {}", label)),
            (None, Some(db)) => Some(format!("Database: {}", db)),
            (None, None) => None,
        }
    }
}

/// System prompt template for the SQL assistant.
const SYSTEM_PROMPT_TEMPLATE: &str = r#"You are a SQL assistant for a PostgreSQL database. Generate SQL queries based on user questions.
{connection}
DATABASE SCHEMA:
{schema}

INSTRUCTIONS:
- Generate only valid PostgreSQL SQL
- Return ONLY the SQL query, no explanations
- Use appropriate JOINs based on foreign keys
- Limit results to 100 rows unless user specifies otherwise
- Never generate DROP DATABASE or similar destructive operations
- If the question cannot be answered with the schema, explain why

OUTPUT FORMAT:
Return the SQL query wrapped in ```sql code blocks.
If you need to explain something, put it before or after the code block."#;

/// Builds the system prompt with the database schema injected.
pub fn build_system_prompt(schema: &Schema) -> String {
    build_system_prompt_with_context(schema, &ConnectionContext::default())
}

/// Builds the system prompt with schema and redacted connection context.
///
/// The connection context is sanitized to exclude sensitive information
/// (passwords, hostnames, usernames) before being included in the prompt.
pub fn build_system_prompt_with_context(schema: &Schema, connection: &ConnectionContext) -> String {
    let schema_text = schema.format_for_llm();
    let connection_text = connection
        .format_for_prompt()
        .map(|c| format!("\n{}\n", c))
        .unwrap_or_default();
    SYSTEM_PROMPT_TEMPLATE
        .replace("{connection}", &connection_text)
        .replace("{schema}", &schema_text)
}

/// Builds the complete message list for an LLM request.
///
/// Combines the system prompt with the conversation history.
pub fn build_messages(schema: &Schema, conversation: &Conversation) -> Vec<Message> {
    let mut messages = Vec::with_capacity(conversation.len() + 1);

    // Add system prompt
    messages.push(Message::system(build_system_prompt(schema)));

    // Add conversation history
    messages.extend(conversation.messages().iter().cloned());

    messages
}

/// Builds messages using a cached system prompt.
pub fn build_messages_cached(
    cache: &mut PromptCache,
    schema: &Schema,
    conversation: &Conversation,
) -> Vec<Message> {
    let mut messages = Vec::with_capacity(conversation.len() + 1);

    // Get cached system prompt
    let system_prompt = cache.get_or_build(schema);
    messages.push(Message::system(system_prompt.to_string()));

    // Add conversation history
    messages.extend(conversation.messages().iter().cloned());

    messages
}

/// Cache for formatted schema prompts.
///
/// Avoids rebuilding the system prompt on every LLM request when the schema
/// hasn't changed.
#[derive(Debug, Default)]
pub struct PromptCache {
    /// Hash of the schema used to build the cached prompt.
    schema_hash: u64,
    /// Hash of the connection context.
    connection_hash: u64,
    /// Cached system prompt.
    system_prompt: Option<Arc<str>>,
}

impl PromptCache {
    /// Creates a new empty prompt cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the cached system prompt, rebuilding if the schema has changed.
    pub fn get_or_build(&mut self, schema: &Schema) -> Arc<str> {
        self.get_or_build_with_context(schema, &ConnectionContext::default())
    }

    /// Gets the cached system prompt with connection context.
    pub fn get_or_build_with_context(
        &mut self,
        schema: &Schema,
        connection: &ConnectionContext,
    ) -> Arc<str> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let schema_hash = schema.content_hash();

        // Hash connection context
        let mut hasher = DefaultHasher::new();
        connection.label.hash(&mut hasher);
        connection.database.hash(&mut hasher);
        let connection_hash = hasher.finish();

        if self.schema_hash != schema_hash
            || self.connection_hash != connection_hash
            || self.system_prompt.is_none()
        {
            self.schema_hash = schema_hash;
            self.connection_hash = connection_hash;
            self.system_prompt = Some(Arc::from(build_system_prompt_with_context(
                schema, connection,
            )));
        }
        Arc::clone(self.system_prompt.as_ref().unwrap())
    }

    /// Invalidates the cache, forcing a rebuild on next access.
    pub fn invalidate(&mut self) {
        self.schema_hash = 0;
        self.connection_hash = 0;
        self.system_prompt = None;
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

    #[test]
    fn test_build_system_prompt_contains_schema() {
        let schema = sample_schema();
        let prompt = build_system_prompt(&schema);

        assert!(prompt.contains("Table: users"));
        assert!(prompt.contains("Table: orders"));
        assert!(prompt.contains("id: integer"));
        assert!(prompt.contains("PostgreSQL"));
    }

    #[test]
    fn test_build_system_prompt_contains_instructions() {
        let schema = Schema::default();
        let prompt = build_system_prompt(&schema);

        assert!(prompt.contains("INSTRUCTIONS:"));
        assert!(prompt.contains("OUTPUT FORMAT:"));
        assert!(prompt.contains("```sql"));
    }

    #[test]
    fn test_build_messages_includes_system_and_conversation() {
        let schema = sample_schema();
        let mut conversation = Conversation::new();
        conversation.add_user("Show me all users");
        conversation.add_assistant("```sql\nSELECT * FROM users;\n```");
        conversation.add_user("Count them");

        let messages = build_messages(&schema, &conversation);

        assert_eq!(messages.len(), 4); // system + 3 conversation messages
        assert_eq!(messages[0].role, crate::llm::types::Role::System);
        assert_eq!(messages[1].role, crate::llm::types::Role::User);
        assert_eq!(messages[2].role, crate::llm::types::Role::Assistant);
        assert_eq!(messages[3].role, crate::llm::types::Role::User);
    }

    #[test]
    fn test_build_messages_empty_conversation() {
        let schema = Schema::default();
        let conversation = Conversation::new();

        let messages = build_messages(&schema, &conversation);

        assert_eq!(messages.len(), 1); // Just system prompt
        assert_eq!(messages[0].role, crate::llm::types::Role::System);
    }

    #[test]
    fn test_connection_context_with_label_and_database() {
        let ctx = ConnectionContext::new(Some("production".to_string()), Some("mydb".to_string()));
        let formatted = ctx.format_for_prompt().unwrap();
        assert_eq!(formatted, "Connection: production (database: mydb)");
    }

    #[test]
    fn test_connection_context_with_label_only() {
        let ctx = ConnectionContext::new(Some("staging".to_string()), None);
        let formatted = ctx.format_for_prompt().unwrap();
        assert_eq!(formatted, "Connection: staging");
    }

    #[test]
    fn test_connection_context_with_database_only() {
        let ctx = ConnectionContext::new(None, Some("testdb".to_string()));
        let formatted = ctx.format_for_prompt().unwrap();
        assert_eq!(formatted, "Database: testdb");
    }

    #[test]
    fn test_connection_context_empty() {
        let ctx = ConnectionContext::default();
        assert!(ctx.format_for_prompt().is_none());
    }

    #[test]
    fn test_build_system_prompt_with_connection_context() {
        let schema = sample_schema();
        let ctx = ConnectionContext::new(Some("prod".to_string()), Some("mydb".to_string()));
        let prompt = build_system_prompt_with_context(&schema, &ctx);

        assert!(prompt.contains("Connection: prod (database: mydb)"));
        assert!(prompt.contains("Table: users"));
        // Verify no sensitive data patterns
        assert!(!prompt.contains("password"));
        assert!(!prompt.contains("host="));
        assert!(!prompt.contains("user="));
    }

    #[test]
    fn test_prompt_cache_with_connection_context() {
        let schema = sample_schema();
        let ctx1 = ConnectionContext::new(Some("prod".to_string()), Some("db1".to_string()));
        let ctx2 = ConnectionContext::new(Some("staging".to_string()), Some("db2".to_string()));

        let mut cache = PromptCache::new();

        let prompt1 = cache.get_or_build_with_context(&schema, &ctx1);
        assert!(prompt1.contains("Connection: prod"));

        // Same context should return cached
        let prompt1_again = cache.get_or_build_with_context(&schema, &ctx1);
        assert!(Arc::ptr_eq(&prompt1, &prompt1_again));

        // Different context should rebuild
        let prompt2 = cache.get_or_build_with_context(&schema, &ctx2);
        assert!(prompt2.contains("Connection: staging"));
        assert!(!Arc::ptr_eq(&prompt1, &prompt2));
    }
}
