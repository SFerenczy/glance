//! LLM client factory.
//!
//! Centralizes provider-specific logic for creating LLM clients.

use crate::error::{GlanceError, Result};
use crate::llm::{
    AnthropicClient, AnthropicConfig, LlmClient, LlmProvider, MockLlmClient, OllamaClient,
    OpenAiClient, OpenAiConfig,
};

/// Creates an LLM client for the given provider.
///
/// If `api_key` is provided, it takes precedence over environment variables.
/// For providers that require an API key (OpenAI, Anthropic), the key is resolved in order:
/// 1. Provided `api_key` parameter
/// 2. Environment variable (`OPENAI_API_KEY` or `ANTHROPIC_API_KEY`)
///
/// Model selection is controlled by environment variables:
/// - `OPENAI_MODEL` (defaults to "gpt-4o")
/// - `ANTHROPIC_MODEL` (defaults to "claude-sonnet-4-20250514")
/// - `OLLAMA_MODEL` (defaults to "llama3.2:3b")
pub fn create_client(provider: LlmProvider, api_key: Option<String>) -> Result<Box<dyn LlmClient>> {
    match provider {
        LlmProvider::OpenAi => {
            let key = api_key
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| {
                    GlanceError::llm(
                        "No API key configured. Use /llm key <key> or set OPENAI_API_KEY.",
                    )
                })?;
            let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
            Ok(Box::new(OpenAiClient::new(OpenAiConfig::new(key, model))?))
        }
        LlmProvider::Anthropic => {
            let key = api_key
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| {
                    GlanceError::llm(
                        "No API key configured. Use /llm key <key> or set ANTHROPIC_API_KEY.",
                    )
                })?;
            let model = std::env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
            Ok(Box::new(AnthropicClient::new(AnthropicConfig::new(
                key, model,
            ))?))
        }
        LlmProvider::Ollama => Ok(Box::new(OllamaClient::from_env()?)),
        LlmProvider::Mock => Ok(Box::new(MockLlmClient::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mock_client() {
        let client = create_client(LlmProvider::Mock, None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_openai_without_key_fails() {
        // Temporarily unset the env var if it exists
        let original = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        let result = create_client(LlmProvider::OpenAi, None);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No API key configured"));

        // Restore
        if let Some(key) = original {
            std::env::set_var("OPENAI_API_KEY", key);
        }
    }

    #[test]
    fn test_create_openai_with_provided_key() {
        let result = create_client(LlmProvider::OpenAi, Some("test-key".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_anthropic_without_key_fails() {
        // Temporarily unset the env var if it exists
        let original = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = create_client(LlmProvider::Anthropic, None);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No API key configured"));

        // Restore
        if let Some(key) = original {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }
    }

    #[test]
    fn test_create_anthropic_with_provided_key() {
        let result = create_client(LlmProvider::Anthropic, Some("test-key".to_string()));
        assert!(result.is_ok());
    }
}
