//! Mock LLM client for testing.
//!
//! Provides deterministic responses based on input patterns.

use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;

use crate::error::Result;
use crate::llm::types::Message;
use crate::llm::LlmClient;

/// Mock LLM client that returns canned responses based on input patterns.
///
/// Used for unit testing without making real API calls.
#[derive(Debug, Clone, Default)]
pub struct MockLlmClient {
    /// Custom response mappings (pattern -> response).
    custom_responses: Vec<(String, String)>,
}

impl MockLlmClient {
    /// Creates a new mock client with default responses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a custom response mapping.
    ///
    /// When the input contains `pattern`, the mock will return `response`.
    pub fn with_response(
        mut self,
        pattern: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        self.custom_responses
            .push((pattern.into(), response.into()));
        self
    }

    /// Generates a mock response based on the input.
    fn mock_response(&self, input: &str) -> String {
        let input_lower = input.to_lowercase();

        // Check custom responses first
        for (pattern, response) in &self.custom_responses {
            if input_lower.contains(&pattern.to_lowercase()) {
                return response.clone();
            }
        }

        // Default pattern matching
        if input_lower.contains("all users") || input_lower.contains("show users") {
            return "```sql\nSELECT * FROM users;\n```".to_string();
        }

        if input_lower.contains("count") && input_lower.contains("orders") {
            return "```sql\nSELECT COUNT(*) FROM orders;\n```".to_string();
        }

        if input_lower.contains("count") && input_lower.contains("users") {
            return "```sql\nSELECT COUNT(*) FROM users;\n```".to_string();
        }

        if input_lower.contains("orders") && input_lower.contains("user") {
            return "```sql\nSELECT o.* FROM orders o\nJOIN users u ON o.user_id = u.id;\n```"
                .to_string();
        }

        if (input_lower.contains("insert") || input_lower.contains("add"))
            && input_lower.contains("user")
        {
            return "```sql\nINSERT INTO users (email, name) VALUES ('test@example.com', 'Test User');\n```".to_string();
        }

        if input_lower.contains("update") && input_lower.contains("user") {
            return "```sql\nUPDATE users SET name = 'Updated Name' WHERE id = 1;\n```"
                .to_string();
        }

        if input_lower.contains("delete") && input_lower.contains("user") {
            return "```sql\nDELETE FROM users WHERE id = 1;\n```".to_string();
        }

        "I don't understand that question. Could you please rephrase it?".to_string()
    }

    /// Extracts the last user message content from a message list.
    fn extract_user_input(messages: &[Message]) -> String {
        messages
            .iter()
            .rev()
            .find(|m| m.role == crate::llm::types::Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let input = Self::extract_user_input(messages);
        Ok(self.mock_response(&input))
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
    ) -> Result<BoxStream<'static, Result<String>>> {
        let response = self.complete(messages).await?;

        // Simulate streaming by yielding chunks
        let chunks: Vec<String> = response
            .chars()
            .collect::<Vec<_>>()
            .chunks(10)
            .map(|c| c.iter().collect())
            .collect();

        let stream = stream::iter(chunks.into_iter().map(Ok));
        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::Message;

    #[tokio::test]
    async fn test_mock_returns_select_all_users() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Show me all users")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("SELECT * FROM users"));
    }

    #[tokio::test]
    async fn test_mock_returns_count_orders() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Count all orders")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("SELECT COUNT(*) FROM orders"));
    }

    #[tokio::test]
    async fn test_mock_returns_unknown_response() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("What is the meaning of life?")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("don't understand"));
    }

    #[tokio::test]
    async fn test_mock_custom_response() {
        let client = MockLlmClient::new()
            .with_response("custom query", "```sql\nSELECT custom FROM table;\n```");

        let messages = vec![Message::user("Run the custom query")];
        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("SELECT custom FROM table"));
    }

    #[tokio::test]
    async fn test_mock_stream() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Show me all users")];

        let mut stream = client.complete_stream(&messages).await.unwrap();

        let mut full_response = String::new();
        while let Some(chunk) = stream.next().await {
            full_response.push_str(&chunk.unwrap());
        }

        assert!(full_response.contains("SELECT * FROM users"));
    }

    #[tokio::test]
    async fn test_mock_insert_user() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Add a new user")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("INSERT INTO users"));
    }

    #[tokio::test]
    async fn test_mock_update_user() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Update the user name")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("UPDATE users"));
    }

    #[tokio::test]
    async fn test_mock_delete_user() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("Delete the user")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("DELETE FROM users"));
    }

    #[tokio::test]
    async fn test_mock_case_insensitive() {
        let client = MockLlmClient::new();
        let messages = vec![Message::user("SHOW ME ALL USERS")];

        let response = client.complete(&messages).await.unwrap();

        assert!(response.contains("SELECT * FROM users"));
    }
}
