//! LLM settings command handlers (/llm provider, /llm model, /llm key).

use std::sync::Arc;

use super::CommandResult;
use crate::commands::router::{LlmKeyArgs, LlmModelArgs, LlmProviderArgs};
use crate::persistence::{self, StateDb};

/// Handle /llm provider command.
pub async fn handle_llm_provider(args: &LlmProviderArgs, state_db: &Arc<StateDb>) -> CommandResult {
    match args {
        LlmProviderArgs::Show => {
            let settings = match persistence::llm_settings::get_llm_settings(state_db.pool()).await
            {
                Ok(s) => s,
                Err(e) => return CommandResult::error(e.to_string()),
            };
            CommandResult::system(format!(
                "Current provider: {}. Use /llm provider <openai|anthropic|ollama> to change.",
                settings.provider
            ))
        }
        LlmProviderArgs::Set(value) => {
            match persistence::llm_settings::set_provider(state_db.pool(), value).await {
                Ok(()) => CommandResult::system(format!(
                    "LLM provider set to '{}'. Conversation cleared.",
                    value
                )),
                Err(e) => CommandResult::error(e.to_string()),
            }
        }
    }
}

/// Handle /llm model command.
pub async fn handle_llm_model(args: &LlmModelArgs, state_db: &Arc<StateDb>) -> CommandResult {
    match args {
        LlmModelArgs::Show => {
            let settings = match persistence::llm_settings::get_llm_settings(state_db.pool()).await
            {
                Ok(s) => s,
                Err(e) => return CommandResult::error(e.to_string()),
            };
            CommandResult::system(format!(
                "Current model: {}. Use /llm model <name> to change.",
                settings.model
            ))
        }
        LlmModelArgs::Set(value) => {
            match persistence::llm_settings::set_model(state_db.pool(), value).await {
                Ok(()) => CommandResult::system(format!("LLM model set to '{}'.", value)),
                Err(e) => CommandResult::error(e.to_string()),
            }
        }
    }
}

/// Handle /llm key command.
pub async fn handle_llm_key(args: &LlmKeyArgs, state_db: &Arc<StateDb>) -> CommandResult {
    match args {
        LlmKeyArgs::Show => {
            let settings = match persistence::llm_settings::get_llm_settings(state_db.pool()).await
            {
                Ok(s) => s,
                Err(e) => return CommandResult::error(e.to_string()),
            };
            let key_status = match settings.api_key_storage {
                persistence::llm_settings::ApiKeyStorage::None => "Not configured".to_string(),
                persistence::llm_settings::ApiKeyStorage::Keyring => {
                    "Configured (stored in keyring)".to_string()
                }
                persistence::llm_settings::ApiKeyStorage::Plaintext => {
                    "Configured (stored in plaintext - not recommended)".to_string()
                }
            };
            CommandResult::system(format!(
                "API key status: {}\n\nUse /llm key <api_key> to set a new key.",
                key_status
            ))
        }
        LlmKeyArgs::Set(value) => {
            let provider = match persistence::llm_settings::get_llm_settings(state_db.pool()).await
            {
                Ok(s) => s.provider,
                Err(e) => return CommandResult::error(e.to_string()),
            };
            match persistence::llm_settings::set_api_key(
                state_db.pool(),
                &provider,
                value,
                state_db.secrets(),
            )
            .await
            {
                Ok(()) => {
                    let masked = persistence::SecretStorage::mask_secret(value);
                    CommandResult::system(format!(
                        "API key set for provider '{}': {}",
                        provider, masked
                    ))
                }
                Err(e) => CommandResult::error(e.to_string()),
            }
        }
    }
}

/// Handle /llm command (show settings).
pub async fn handle_llm_settings(state_db: &Arc<StateDb>) -> CommandResult {
    let settings = match persistence::llm_settings::get_llm_settings(state_db.pool()).await {
        Ok(s) => s,
        Err(e) => return CommandResult::error(e.to_string()),
    };
    CommandResult::system(format!(
        "LLM settings:\n  Provider: {}\n  Model: {}\n\nCommands:\n  /llm provider <name>\n  /llm model <name>\n  /llm key",
        settings.provider, settings.model
    ))
}
