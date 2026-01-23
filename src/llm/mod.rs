//! LLM integration for Glance.
//!
//! Provides traits and implementations for communicating with various LLM providers.

#![allow(dead_code)] // Components will be used in Phase 8 integration
#![allow(unused_imports)] // Re-exports for external use

pub mod anthropic;
pub mod factory;
pub mod manager;
pub mod mock;
pub mod ollama;
pub mod openai;
pub mod parser;
pub mod prompt;
pub mod service;
pub mod tools;
pub mod types;

pub use anthropic::{AnthropicClient, AnthropicConfig};
pub use factory::{
    create_client, create_client_from_config, create_client_from_persistence,
    create_client_with_overrides, resolve_config, LlmConfigBuilder, RuntimeLlmConfig,
};
pub use manager::LlmManager;
pub use mock::MockLlmClient;
pub use ollama::{OllamaClient, OllamaConfig};
pub use openai::{OpenAiClient, OpenAiConfig};
pub use parser::{parse_llm_response, ParsedResponse};
pub use prompt::{build_messages, build_messages_cached, build_system_prompt, PromptCache};
pub use service::{LlmResult, LlmService, ToolContext};
pub use tools::{
    format_saved_queries_for_llm, get_tool_definitions, ListSavedQueriesInput, SavedQueryOutput,
    ToolDefinition,
};
pub use types::{Conversation, LlmResponse, Message, Role, ToolCall, ToolResult};

use async_trait::async_trait;
use futures::stream::BoxStream;
use std::str::FromStr;

use crate::error::Result;

/// Trait for LLM clients that can generate completions.
///
/// Implementations must be thread-safe (Send + Sync) to support async operations.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generates a completion for the given messages.
    ///
    /// Returns the complete response as a single string.
    async fn complete(&self, messages: &[Message]) -> Result<String>;

    /// Generates a streaming completion for the given messages.
    ///
    /// Returns a stream of response chunks as they arrive.
    async fn complete_stream(
        &self,
        messages: &[Message],
    ) -> Result<BoxStream<'static, Result<String>>>;

    /// Generates a completion with tool support.
    ///
    /// Returns an LlmResponse that may contain tool calls.
    /// Default implementation wraps `complete()` for backwards compatibility.
    async fn complete_with_tools(
        &self,
        messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let content = self.complete(messages).await?;
        Ok(LlmResponse::text(content))
    }

    /// Continues a conversation after tool results are provided.
    ///
    /// The `assistant_tool_calls` parameter contains the original tool calls from the
    /// assistant's response, which some providers (like OpenAI) require to be included
    /// in the message history before the tool results.
    ///
    /// Default implementation just calls complete() with the messages.
    async fn continue_with_tool_results(
        &self,
        messages: &[Message],
        _assistant_tool_calls: &[ToolCall],
        _tool_results: &[ToolResult],
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let content = self.complete(messages).await?;
        Ok(LlmResponse::text(content))
    }
}

/// LLM provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProvider {
    /// OpenAI (GPT-4, etc.)
    #[default]
    OpenAi,
    /// Anthropic (Claude)
    Anthropic,
    /// Local Ollama instance
    Ollama,
    /// Mock client for testing (no API key required)
    Mock,
}

impl LlmProvider {
    /// Returns the provider as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Ollama => "ollama",
            Self::Mock => "mock",
        }
    }
}

impl FromStr for LlmProvider {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "ollama" => Ok(Self::Ollama),
            "mock" => Ok(Self::Mock),
            _ => Err(format!("Unknown LLM provider: {}", s)),
        }
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert_eq!(
            "openai".parse::<LlmProvider>().unwrap(),
            LlmProvider::OpenAi
        );
        assert_eq!(
            "OpenAI".parse::<LlmProvider>().unwrap(),
            LlmProvider::OpenAi
        );
        assert_eq!(
            "anthropic".parse::<LlmProvider>().unwrap(),
            LlmProvider::Anthropic
        );
        assert_eq!(
            "ollama".parse::<LlmProvider>().unwrap(),
            LlmProvider::Ollama
        );
        assert!("unknown".parse::<LlmProvider>().is_err());
    }

    #[test]
    fn test_provider_as_str() {
        assert_eq!(LlmProvider::OpenAi.as_str(), "openai");
        assert_eq!(LlmProvider::Anthropic.as_str(), "anthropic");
        assert_eq!(LlmProvider::Ollama.as_str(), "ollama");
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(format!("{}", LlmProvider::OpenAi), "openai");
    }

    #[test]
    fn test_provider_default() {
        assert_eq!(LlmProvider::default(), LlmProvider::OpenAi);
    }

    #[tokio::test]
    async fn test_mock_client_implements_trait() {
        let client: Box<dyn LlmClient> = Box::new(MockLlmClient::new());
        let messages = vec![Message::user("Show me all users")];
        let response = client.complete(&messages).await.unwrap();
        assert!(response.contains("SELECT"));
    }
}
