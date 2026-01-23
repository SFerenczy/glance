//! LLM client factory.
//!
//! Centralizes provider-specific logic for creating LLM clients.
//!
//! Resolution order for all settings: CLI override → persisted settings → env → defaults.

use std::sync::Arc;

use crate::error::{GlanceError, Result};
use crate::llm::{
    AnthropicClient, AnthropicConfig, LlmClient, LlmProvider, MockLlmClient, OllamaClient,
    OllamaConfig, OpenAiClient, OpenAiConfig,
};
use crate::persistence::{self, StateDb};

/// Runtime LLM configuration with unified resolution.
///
/// This struct captures all LLM settings after resolution. The resolution order is:
/// 1. CLI override (highest priority)
/// 2. Persisted settings (from state database)
/// 3. Environment variables
/// 4. Provider-specific defaults (lowest priority)
#[derive(Debug, Clone, Default)]
pub struct RuntimeLlmConfig {
    /// LLM provider.
    pub provider: LlmProvider,
    /// Model name.
    pub model: Option<String>,
    /// API key.
    pub api_key: Option<String>,
    /// Base URL (for Ollama or custom endpoints).
    pub base_url: Option<String>,
}

/// Builder for RuntimeLlmConfig with layered resolution.
#[derive(Debug, Clone, Default)]
pub struct LlmConfigBuilder {
    cli_provider: Option<LlmProvider>,
    cli_model: Option<String>,
    cli_api_key: Option<String>,
    cli_base_url: Option<String>,
    persisted_provider: Option<String>,
    persisted_model: Option<String>,
    persisted_api_key: Option<String>,
}

impl LlmConfigBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets CLI overrides (highest priority).
    pub fn with_cli_overrides(
        mut self,
        provider: Option<LlmProvider>,
        model: Option<String>,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        self.cli_provider = provider;
        self.cli_model = model;
        self.cli_api_key = api_key;
        self.cli_base_url = base_url;
        self
    }

    /// Sets persisted settings from state database.
    pub fn with_persisted(
        mut self,
        provider: Option<String>,
        model: Option<String>,
        api_key: Option<String>,
    ) -> Self {
        self.persisted_provider = provider;
        self.persisted_model = model;
        self.persisted_api_key = api_key;
        self
    }

    /// Loads persisted settings from state database.
    pub async fn load_from_persistence(mut self, state_db: Option<&Arc<StateDb>>) -> Result<Self> {
        if let Some(db) = state_db {
            let settings = persistence::llm_settings::get_llm_settings(db.pool()).await?;
            self.persisted_provider = Some(settings.provider.clone());
            self.persisted_model = if settings.model.is_empty() {
                None
            } else {
                Some(settings.model)
            };
            self.persisted_api_key =
                persistence::llm_settings::get_api_key(db.pool(), &settings.provider, db.secrets())
                    .await?;
        }
        Ok(self)
    }

    /// Builds the final RuntimeLlmConfig by resolving all layers.
    pub fn build(self) -> RuntimeLlmConfig {
        let provider = self.resolve_provider();
        let model = self.resolve_model(&provider);
        let api_key = self.resolve_api_key(&provider);
        let base_url = self.resolve_base_url(&provider);

        RuntimeLlmConfig {
            provider,
            model,
            api_key,
            base_url,
        }
    }

    fn resolve_provider(&self) -> LlmProvider {
        // CLI → persisted → env → default
        if let Some(p) = self.cli_provider {
            return p;
        }
        if let Some(ref p) = self.persisted_provider {
            if let Ok(provider) = p.parse::<LlmProvider>() {
                return provider;
            }
        }
        if let Ok(p) = std::env::var("GLANCE_LLM_PROVIDER") {
            if let Ok(provider) = p.parse::<LlmProvider>() {
                return provider;
            }
        }
        LlmProvider::default()
    }

    fn resolve_model(&self, provider: &LlmProvider) -> Option<String> {
        // CLI → persisted → env → None (let create_client use its default)
        if let Some(ref m) = self.cli_model {
            return Some(m.clone());
        }
        if let Some(ref m) = self.persisted_model {
            return Some(m.clone());
        }
        // Check provider-specific env vars
        let env_var = match provider {
            LlmProvider::OpenAi => "OPENAI_MODEL",
            LlmProvider::Anthropic => "ANTHROPIC_MODEL",
            LlmProvider::Ollama => "OLLAMA_MODEL",
            LlmProvider::Mock => return None,
        };
        std::env::var(env_var).ok()
    }

    fn resolve_api_key(&self, provider: &LlmProvider) -> Option<String> {
        // CLI → persisted → env
        if let Some(ref k) = self.cli_api_key {
            return Some(k.clone());
        }
        if let Some(ref k) = self.persisted_api_key {
            return Some(k.clone());
        }
        // Check provider-specific env vars
        let env_var = match provider {
            LlmProvider::OpenAi => "OPENAI_API_KEY",
            LlmProvider::Anthropic => "ANTHROPIC_API_KEY",
            LlmProvider::Ollama | LlmProvider::Mock => return None,
        };
        std::env::var(env_var).ok()
    }

    fn resolve_base_url(&self, provider: &LlmProvider) -> Option<String> {
        // CLI → env
        if let Some(ref u) = self.cli_base_url {
            return Some(u.clone());
        }
        match provider {
            LlmProvider::Ollama => std::env::var("OLLAMA_URL").ok(),
            LlmProvider::OpenAi => std::env::var("OPENAI_BASE_URL").ok(),
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_BASE_URL").ok(),
            LlmProvider::Mock => None,
        }
    }
}

/// Creates an LLM client from a RuntimeLlmConfig.
///
/// This is the primary entry point for creating LLM clients. The config should
/// already have all settings resolved via LlmConfigBuilder.
pub fn create_client_from_config(config: &RuntimeLlmConfig) -> Result<Box<dyn LlmClient>> {
    match config.provider {
        LlmProvider::OpenAi => {
            let key = config.api_key.clone().ok_or_else(|| {
                GlanceError::llm("No API key configured. Use /llm key <key> or set OPENAI_API_KEY.")
            })?;
            let model = config.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
            Ok(Box::new(OpenAiClient::new(OpenAiConfig::new(key, model))?))
        }
        LlmProvider::Anthropic => {
            let key = config.api_key.clone().ok_or_else(|| {
                GlanceError::llm(
                    "No API key configured. Use /llm key <key> or set ANTHROPIC_API_KEY.",
                )
            })?;
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
            Ok(Box::new(AnthropicClient::new(AnthropicConfig::new(
                key, model,
            ))?))
        }
        LlmProvider::Ollama => {
            let base_url = config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "llama3.2:3b".to_string());
            Ok(Box::new(OllamaClient::new(
                OllamaConfig::new(model).with_url(base_url),
            )?))
        }
        LlmProvider::Mock => Ok(Box::new(MockLlmClient::new())),
    }
}

/// Creates an LLM client for the given provider (legacy API).
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
    let config = RuntimeLlmConfig {
        provider,
        model,
        api_key,
        base_url: None,
    };
    // For legacy API, we need to check env vars if api_key/model not provided
    let resolved_config = RuntimeLlmConfig {
        provider: config.provider,
        model: config.model.or_else(|| {
            let env_var = match provider {
                LlmProvider::OpenAi => "OPENAI_MODEL",
                LlmProvider::Anthropic => "ANTHROPIC_MODEL",
                LlmProvider::Ollama => "OLLAMA_MODEL",
                LlmProvider::Mock => return None,
            };
            std::env::var(env_var).ok()
        }),
        api_key: config.api_key.or_else(|| {
            let env_var = match provider {
                LlmProvider::OpenAi => "OPENAI_API_KEY",
                LlmProvider::Anthropic => "ANTHROPIC_API_KEY",
                LlmProvider::Ollama | LlmProvider::Mock => return None,
            };
            std::env::var(env_var).ok()
        }),
        base_url: match provider {
            LlmProvider::Ollama => std::env::var("OLLAMA_URL").ok(),
            _ => None,
        },
    };
    create_client_from_config(&resolved_config)
}

/// Creates an LLM client using settings from persistence.
///
/// This is the primary entry point for creating LLM clients in the application.
/// It uses LlmConfigBuilder to resolve settings with proper precedence.
///
/// Resolution order:
/// 1. CLI override (provider parameter, if not default)
/// 2. Persisted settings from state database
/// 3. Environment variables
/// 4. Provider-specific defaults
pub async fn create_client_from_persistence(
    provider: LlmProvider,
    state_db: Option<&Arc<StateDb>>,
) -> Result<Box<dyn LlmClient>> {
    // Use the CLI provider as override if it's not the default
    let cli_provider = if provider != LlmProvider::default() {
        Some(provider)
    } else {
        None
    };

    let config = LlmConfigBuilder::new()
        .with_cli_overrides(cli_provider, None, None, None)
        .load_from_persistence(state_db)
        .await?
        .build();

    create_client_from_config(&config)
}

/// Creates an LLM client with full control over resolution.
///
/// This is the most flexible entry point, allowing CLI overrides for all settings.
pub async fn create_client_with_overrides(
    cli_provider: Option<LlmProvider>,
    cli_model: Option<String>,
    cli_api_key: Option<String>,
    cli_base_url: Option<String>,
    state_db: Option<&Arc<StateDb>>,
) -> Result<Box<dyn LlmClient>> {
    let config = LlmConfigBuilder::new()
        .with_cli_overrides(cli_provider, cli_model, cli_api_key, cli_base_url)
        .load_from_persistence(state_db)
        .await?
        .build();

    create_client_from_config(&config)
}

/// Resolves LLM configuration without creating a client.
///
/// Useful for inspecting what settings would be used.
pub async fn resolve_config(
    cli_provider: Option<LlmProvider>,
    cli_model: Option<String>,
    state_db: Option<&Arc<StateDb>>,
) -> Result<RuntimeLlmConfig> {
    let config = LlmConfigBuilder::new()
        .with_cli_overrides(cli_provider, cli_model, None, None)
        .load_from_persistence(state_db)
        .await?
        .build();

    Ok(config)
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

    #[test]
    fn test_config_builder_cli_overrides_persisted() {
        let config = LlmConfigBuilder::new()
            .with_cli_overrides(
                Some(LlmProvider::Anthropic),
                Some("cli-model".to_string()),
                Some("cli-key".to_string()),
                None,
            )
            .with_persisted(
                Some("openai".to_string()),
                Some("persisted-model".to_string()),
                Some("persisted-key".to_string()),
            )
            .build();

        assert_eq!(config.provider, LlmProvider::Anthropic);
        assert_eq!(config.model, Some("cli-model".to_string()));
        assert_eq!(config.api_key, Some("cli-key".to_string()));
    }

    #[test]
    fn test_config_builder_persisted_used_when_no_cli() {
        let config = LlmConfigBuilder::new()
            .with_cli_overrides(None, None, None, None)
            .with_persisted(
                Some("anthropic".to_string()),
                Some("persisted-model".to_string()),
                Some("persisted-key".to_string()),
            )
            .build();

        assert_eq!(config.provider, LlmProvider::Anthropic);
        assert_eq!(config.model, Some("persisted-model".to_string()));
        assert_eq!(config.api_key, Some("persisted-key".to_string()));
    }

    #[test]
    fn test_config_builder_env_fallback() {
        // Save and clear env vars
        let orig_provider = std::env::var("GLANCE_LLM_PROVIDER").ok();
        let orig_model = std::env::var("OPENAI_MODEL").ok();
        let orig_key = std::env::var("OPENAI_API_KEY").ok();

        std::env::set_var("GLANCE_LLM_PROVIDER", "openai");
        std::env::set_var("OPENAI_MODEL", "env-model");
        std::env::set_var("OPENAI_API_KEY", "env-key");

        let config = LlmConfigBuilder::new()
            .with_cli_overrides(None, None, None, None)
            .with_persisted(None, None, None)
            .build();

        assert_eq!(config.provider, LlmProvider::OpenAi);
        assert_eq!(config.model, Some("env-model".to_string()));
        assert_eq!(config.api_key, Some("env-key".to_string()));

        // Restore env vars
        match orig_provider {
            Some(v) => std::env::set_var("GLANCE_LLM_PROVIDER", v),
            None => std::env::remove_var("GLANCE_LLM_PROVIDER"),
        }
        match orig_model {
            Some(v) => std::env::set_var("OPENAI_MODEL", v),
            None => std::env::remove_var("OPENAI_MODEL"),
        }
        match orig_key {
            Some(v) => std::env::set_var("OPENAI_API_KEY", v),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
    }

    #[test]
    fn test_config_builder_defaults() {
        // Clear env vars that might interfere
        let orig_provider = std::env::var("GLANCE_LLM_PROVIDER").ok();
        std::env::remove_var("GLANCE_LLM_PROVIDER");

        let config = LlmConfigBuilder::new()
            .with_cli_overrides(None, None, None, None)
            .with_persisted(None, None, None)
            .build();

        assert_eq!(config.provider, LlmProvider::default());

        // Restore
        if let Some(v) = orig_provider {
            std::env::set_var("GLANCE_LLM_PROVIDER", v);
        }
    }

    #[test]
    fn test_create_client_from_config_mock() {
        let config = RuntimeLlmConfig {
            provider: LlmProvider::Mock,
            model: None,
            api_key: None,
            base_url: None,
        };
        let client = create_client_from_config(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_client_from_config_openai() {
        let config = RuntimeLlmConfig {
            provider: LlmProvider::OpenAi,
            model: Some("gpt-4".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: None,
        };
        let client = create_client_from_config(&config);
        assert!(client.is_ok());
    }
}
