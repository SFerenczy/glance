//! Prompt construction for LLM requests.
//!
//! Builds system prompts with database schema context.

use crate::db::Schema;
use crate::llm::types::{Conversation, Message};
use std::sync::Arc;

/// System prompt template for the SQL assistant.
const SYSTEM_PROMPT_TEMPLATE: &str = r#"You are a SQL assistant for a PostgreSQL database. Generate SQL queries based on user questions.

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
    let schema_text = schema.format_for_llm();
    SYSTEM_PROMPT_TEMPLATE.replace("{schema}", &schema_text)
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
        let hash = schema.content_hash();
        if self.schema_hash != hash || self.system_prompt.is_none() {
            self.schema_hash = hash;
            self.system_prompt = Some(Arc::from(build_system_prompt(schema)));
        }
        Arc::clone(self.system_prompt.as_ref().unwrap())
    }

    /// Invalidates the cache, forcing a rebuild on next access.
    pub fn invalidate(&mut self) {
        self.schema_hash = 0;
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
}
