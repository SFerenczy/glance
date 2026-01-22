//! LLM client factory.
//!
//! Centralizes provider-specific logic for creating LLM clients.

use std::sync::Arc;

use crate::error::{GlanceError, Result};
use crate::llm::{
    AnthropicClient, AnthropicConfig, LlmClient, LlmProvider, MockLlmClient, OllamaClient,
    OllamaConfig, OpenAiClient, OpenAiConfig,
};
use crate::persistence::{self, StateDb};

/// Creates an LLM client for the given provider.
///
/// If `api_key` is provided, it takes precedence over environment variables.
/// For providers that require an API key (OpenAI, Anthropic), the key is resolved in order:
/// 1. Provided `api_key` parameter
/// 2. Environment variable (`OPENAI_API_KEY` or `ANTHROPIC_API_KEY`)
///
/// If `model` is provided, it takes precedence over environment variables.
/// Model fallback order:
/// 1. Provided `model` parameter
/// 2. Environment variable (`OPENAI_MODEL`, `ANTHROPIC_MODEL`, or `OLLAMA_MODEL`)
/// 3. Provider-specific default
pub fn create_client(
    provider: LlmProvider,
    api_key: Option<String>,
    model: Option<String>,
) -> Result<Box<dyn LlmClient>> {
    match provider {
        LlmProvider::OpenAi => {
            let key = api_key
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| {
                    GlanceError::llm(
                        "No API key configured. Use /llm key <key> or set OPENAI_API_KEY.",
                    )
                })?;
            let model = model
                .clone()
                .or_else(|| std::env::var("OPENAI_MODEL").ok())
                .unwrap_or_else(|| "gpt-4o".to_string());
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
            let model = model
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
                .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
            Ok(Box::new(AnthropicClient::new(AnthropicConfig::new(
                key, model,
            ))?))
        }
        LlmProvider::Ollama => {
            let base_url = std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            let model = model
                .or_else(|| std::env::var("OLLAMA_MODEL").ok())
                .unwrap_or_else(|| "llama3.2:3b".to_string());
            Ok(Box::new(OllamaClient::new(
                OllamaConfig::new(model).with_url(base_url),
            )?))
        }
        LlmProvider::Mock => Ok(Box::new(MockLlmClient::new())),
    }
}

/// Creates an LLM client using settings from persistence.
///
/// This is the primary entry point for creating LLM clients in the application.
/// It encapsulates all persistence lookup logic, keeping the orchestrator simple.
///
/// Resolution order:
/// 1. Load provider and model from persistence (or use defaults)
/// 2. Load API key from persistence (keyring or plaintext)
/// 3. Fall back to environment variables if persistence has no key
/// 4. Delegate to `create_client` for actual client construction
pub async fn create_client_from_persistence(
    provider: LlmProvider,
    state_db: Option<&Arc<StateDb>>,
) -> Result<Box<dyn LlmClient>> {
    let (persisted_key, persisted_model) = if let Some(db) = state_db {
        let settings = persistence::llm_settings::get_llm_settings(db.pool()).await?;
        let key =
            persistence::llm_settings::get_api_key(db.pool(), &settings.provider, db.secrets())
                .await?;
        let model = if settings.model.is_empty() {
            None
        } else {
            Some(settings.model)
        };
        (key, model)
    } else {
        (None, None)
    };

    create_client(provider, persisted_key, persisted_model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mock_client() {
        let client = create_client(LlmProvider::Mock, None, None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_openai_without_key_fails() {
        // Temporarily unset the env var if it exists
        let original = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        let result = create_client(LlmProvider::OpenAi, None, None);
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
        let result = create_client(LlmProvider::OpenAi, Some("test-key".to_string()), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_anthropic_without_key_fails() {
        // Temporarily unset the env var if it exists
        let original = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = create_client(LlmProvider::Anthropic, None, None);
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
        let result = create_client(LlmProvider::Anthropic, Some("test-key".to_string()), None);
        assert!(result.is_ok());
    }
}
