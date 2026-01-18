//! LLM integration for Glance.
//!
//! Provides traits and implementations for communicating with various LLM providers.

#![allow(dead_code)] // Components will be used in Phase 8 integration
#![allow(unused_imports)] // Re-exports for external use

pub mod anthropic;
pub mod mock;
pub mod ollama;
pub mod openai;
pub mod parser;
pub mod prompt;
pub mod tools;
pub mod types;

pub use anthropic::{AnthropicClient, AnthropicConfig};
pub use mock::MockLlmClient;
pub use ollama::{OllamaClient, OllamaConfig};
pub use openai::{OpenAiClient, OpenAiConfig};
pub use parser::{parse_llm_response, ParsedResponse};
pub use prompt::{build_messages, build_system_prompt};
pub use tools::{
    format_saved_queries_for_llm, get_tool_definitions, ListSavedQueriesInput, SavedQueryOutput,
    ToolDefinition,
};
pub use types::{Conversation, Message, Role};

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
