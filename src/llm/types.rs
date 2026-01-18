//! Message types for LLM communication.
//!
//! Defines the core types used for building conversations with LLM providers.

use serde::{Deserialize, Serialize};

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call (used to match results).
    pub id: String,
    /// Name of the tool to call.
    pub name: String,
    /// JSON arguments for the tool.
    pub arguments: String,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// ID of the tool call this result is for.
    pub tool_call_id: String,
    /// The result content (typically JSON).
    pub content: String,
}

/// Response from an LLM that may include tool calls.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// Text content from the LLM (may be empty if only tool calls).
    pub content: String,
    /// Tool calls requested by the LLM.
    pub tool_calls: Vec<ToolCall>,
}

impl LlmResponse {
    /// Creates a response with only text content.
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
        }
    }

    /// Creates a response with tool calls.
    pub fn with_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content: content.into(),
            tool_calls,
        }
    }

    /// Returns true if this response contains tool calls.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System message providing context and instructions.
    System,
    /// User message (human input).
    User,
    /// Assistant message (LLM response).
    Assistant,
}

impl Role {
    /// Returns the role as a string for API requests.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender.
    pub role: Role,
    /// The content of the message.
    pub content: String,
}

impl Message {
    /// Creates a new message with the given role and content.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Creates a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Creates a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Creates an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }
}

/// A conversation consisting of multiple messages.
///
/// Maintains conversation history for context in LLM requests.
#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<Message>,
    /// Maximum number of exchanges to keep (each exchange = user + assistant).
    max_exchanges: usize,
}

impl Conversation {
    /// Creates a new empty conversation.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_exchanges: 10, // Default per FR-3.4
        }
    }

    /// Creates a conversation with a custom max exchanges limit.
    pub fn with_max_exchanges(max_exchanges: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_exchanges,
        }
    }

    /// Adds a message to the conversation.
    pub fn add(&mut self, message: Message) {
        self.messages.push(message);
        self.trim_to_limit();
    }

    /// Adds a user message to the conversation.
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.add(Message::user(content));
    }

    /// Adds an assistant message to the conversation.
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.add(Message::assistant(content));
    }

    /// Returns all messages in the conversation.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Clears all messages from the conversation.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Returns the number of messages in the conversation.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns true if the conversation has no messages.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Trims the conversation to keep only the most recent exchanges.
    fn trim_to_limit(&mut self) {
        // An exchange is a user message followed by an assistant message.
        // We want to keep at most max_exchanges complete exchanges.
        // We preserve system messages at the start.

        // Find where non-system messages start
        let start_idx = self
            .messages
            .iter()
            .position(|m| m.role != Role::System)
            .unwrap_or(self.messages.len());

        // Count complete exchanges (user followed by assistant)
        let non_system_messages = &self.messages[start_idx..];
        let mut exchange_count = 0;
        let mut i = 0;
        while i + 1 < non_system_messages.len() {
            if non_system_messages[i].role == Role::User
                && non_system_messages[i + 1].role == Role::Assistant
            {
                exchange_count += 1;
                i += 2;
            } else {
                i += 1;
            }
        }

        // Remove oldest exchanges until we're within the limit
        while exchange_count > self.max_exchanges {
            // Find the first complete exchange after system messages
            let mut removed = false;
            for i in start_idx..self.messages.len().saturating_sub(1) {
                if self.messages[i].role == Role::User
                    && self.messages[i + 1].role == Role::Assistant
                {
                    // Remove this exchange (2 messages)
                    self.messages.remove(i);
                    self.messages.remove(i);
                    exchange_count -= 1;
                    removed = true;
                    break;
                }
            }
            if !removed {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_as_str() {
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
    }

    #[test]
    fn test_message_constructors() {
        let system = Message::system("You are a helpful assistant.");
        assert_eq!(system.role, Role::System);
        assert_eq!(system.content, "You are a helpful assistant.");

        let user = Message::user("Hello!");
        assert_eq!(user.role, Role::User);
        assert_eq!(user.content, "Hello!");

        let assistant = Message::assistant("Hi there!");
        assert_eq!(assistant.role, Role::Assistant);
        assert_eq!(assistant.content, "Hi there!");
    }

    #[test]
    fn test_conversation_add_messages() {
        let mut conv = Conversation::new();
        assert!(conv.is_empty());

        conv.add_user("Hello");
        assert_eq!(conv.len(), 1);

        conv.add_assistant("Hi!");
        assert_eq!(conv.len(), 2);

        let messages = conv.messages();
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_conversation_clear() {
        let mut conv = Conversation::new();
        conv.add_user("Hello");
        conv.add_assistant("Hi!");
        assert_eq!(conv.len(), 2);

        conv.clear();
        assert!(conv.is_empty());
    }

    #[test]
    fn test_conversation_trim_to_limit() {
        let mut conv = Conversation::with_max_exchanges(2);

        // Add 3 exchanges
        for i in 0..3 {
            conv.add_user(format!("Question {}", i));
            conv.add_assistant(format!("Answer {}", i));
        }

        // Should have trimmed to 2 exchanges (4 messages)
        assert!(conv.len() <= 4);
    }

    #[test]
    fn test_role_serialization() {
        let role = Role::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Role::User);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }
}
