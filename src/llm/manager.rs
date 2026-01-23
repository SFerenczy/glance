//! LLM client manager for centralized provider/key/model management.
//!
//! This module provides a lightweight manager that owns LLM client lifecycle,
//! handling provider/key/model swaps and secret retrieval from persistence.
//!
//! The orchestrator becomes a consumer-only: it requests the current client
//! and subscribes to change notifications instead of reaching into persistence.

use std::sync::Arc;

use crate::error::Result;
use crate::persistence::StateDb;

use super::{
    factory::{LlmConfigBuilder, RuntimeLlmConfig},
    LlmClient, LlmProvider,
};

/// Manages LLM client lifecycle and configuration.
///
/// This manager centralizes all LLM-related state and provides a clean interface
/// for the orchestrator to consume without needing to understand persistence details.
pub struct LlmManager {
    /// Current LLM client.
    client: Box<dyn LlmClient>,
    /// Current resolved configuration.
    config: RuntimeLlmConfig,
    /// State database for persistence (optional).
    state_db: Option<Arc<StateDb>>,
    /// CLI overrides that persist across rebuilds.
    cli_provider: Option<LlmProvider>,
    cli_model: Option<String>,
    cli_api_key: Option<String>,
    cli_base_url: Option<String>,
}

impl LlmManager {
    /// Creates a new LLM manager with the given initial configuration.
    pub async fn new(
        cli_provider: Option<LlmProvider>,
        cli_model: Option<String>,
        cli_api_key: Option<String>,
        cli_base_url: Option<String>,
        state_db: Option<Arc<StateDb>>,
    ) -> Result<Self> {
        let config = LlmConfigBuilder::new()
            .with_cli_overrides(
                cli_provider,
                cli_model.clone(),
                cli_api_key.clone(),
                cli_base_url.clone(),
            )
            .load_from_persistence(state_db.as_ref())
            .await?
            .build();

        let client = super::factory::create_client_from_config(&config)?;

        tracing::info!(
            provider = %config.provider,
            model = config.model.as_deref().unwrap_or("default"),
            has_api_key = config.api_key.is_some(),
            "LLM manager initialized"
        );

        Ok(Self {
            client,
            config,
            state_db,
            cli_provider,
            cli_model,
            cli_api_key,
            cli_base_url,
        })
    }

    /// Creates a manager with a mock client for testing.
    #[allow(dead_code)]
    pub fn mock() -> Self {
        Self {
            client: Box::new(super::MockLlmClient::new()),
            config: RuntimeLlmConfig {
                provider: LlmProvider::Mock,
                model: None,
                api_key: None,
                base_url: None,
            },
            state_db: None,
            cli_provider: Some(LlmProvider::Mock),
            cli_model: None,
            cli_api_key: None,
            cli_base_url: None,
        }
    }

    /// Returns a reference to the current LLM client.
    pub fn client(&self) -> &dyn LlmClient {
        self.client.as_ref()
    }

    /// Returns the current resolved configuration.
    pub fn config(&self) -> &RuntimeLlmConfig {
        &self.config
    }

    /// Returns the current provider.
    pub fn provider(&self) -> LlmProvider {
        self.config.provider
    }

    /// Returns the current model name.
    pub fn model(&self) -> Option<&str> {
        self.config.model.as_deref()
    }

    /// Rebuilds the LLM client with current settings from persistence.
    ///
    /// This should be called after settings are changed via /llm commands.
    /// CLI overrides are preserved across rebuilds.
    pub async fn rebuild(&mut self) -> Result<()> {
        let config = LlmConfigBuilder::new()
            .with_cli_overrides(
                self.cli_provider,
                self.cli_model.clone(),
                self.cli_api_key.clone(),
                self.cli_base_url.clone(),
            )
            .load_from_persistence(self.state_db.as_ref())
            .await?
            .build();

        let client = super::factory::create_client_from_config(&config)?;

        tracing::info!(
            provider = %config.provider,
            model = config.model.as_deref().unwrap_or("default"),
            "LLM client rebuilt"
        );

        self.client = client;
        self.config = config;
        Ok(())
    }

    /// Takes ownership of the client, consuming the manager.
    ///
    /// This is useful for transferring the client to another owner.
    #[allow(dead_code)]
    pub fn into_client(self) -> Box<dyn LlmClient> {
        self.client
    }

    /// Replaces the current client with a new one.
    ///
    /// This is a low-level operation; prefer `rebuild()` for normal use.
    pub fn set_client(&mut self, client: Box<dyn LlmClient>) {
        self.client = client;
    }

    /// Returns a reference to the state database, if available.
    pub fn state_db(&self) -> Option<&Arc<StateDb>> {
        self.state_db.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_manager() {
        let manager = LlmManager::mock();
        assert_eq!(manager.provider(), LlmProvider::Mock);
        assert!(manager.model().is_none());
    }

    #[tokio::test]
    async fn test_manager_with_cli_overrides() {
        // Clear env vars that might interfere
        let orig_key = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        let manager = LlmManager::new(
            Some(LlmProvider::OpenAi),
            Some("gpt-4".to_string()),
            Some("test-key".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(manager.provider(), LlmProvider::OpenAi);
        assert_eq!(manager.model(), Some("gpt-4"));

        // Restore
        if let Some(key) = orig_key {
            std::env::set_var("OPENAI_API_KEY", key);
        }
    }
}
