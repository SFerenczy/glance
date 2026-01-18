//! LLM provider and settings persistence.
//!
//! Stores LLM provider, model, and API key configuration.

#![allow(dead_code)]

use crate::error::{GlanceError, Result};
use crate::persistence::secrets::SecretStorage;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;

/// API key storage method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyStorage {
    /// No API key stored.
    None,
    /// API key stored in OS keyring.
    Keyring,
    /// API key stored as plaintext (with user consent).
    Plaintext,
}

impl ApiKeyStorage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Keyring => "keyring",
            Self::Plaintext => "plaintext",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "keyring" => Self::Keyring,
            "plaintext" => Self::Plaintext,
            _ => Self::None,
        }
    }
}

/// Raw database row for LLM settings.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
struct LlmSettingsRow {
    provider: String,
    model: String,
    api_key_storage: String,
    api_key_plaintext: Option<String>,
    updated_at: String,
}

/// LLM provider settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub provider: String,
    pub model: String,
    pub api_key_storage: ApiKeyStorage,
    pub updated_at: String,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-5".to_string(),
            api_key_storage: ApiKeyStorage::None,
            updated_at: String::new(),
        }
    }
}

/// Gets the current LLM settings.
pub async fn get_llm_settings(pool: &SqlitePool) -> Result<LlmSettings> {
    let row: Option<LlmSettingsRow> = sqlx::query_as(
        "SELECT provider, model, api_key_storage, api_key_plaintext, updated_at FROM llm_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to get LLM settings: {e}")))?;

    Ok(row
        .map(|r| LlmSettings {
            provider: r.provider,
            model: r.model,
            api_key_storage: ApiKeyStorage::from_str(&r.api_key_storage),
            updated_at: r.updated_at,
        })
        .unwrap_or_default())
}

/// Updates the LLM provider.
pub async fn set_provider(pool: &SqlitePool, provider: &str) -> Result<()> {
    let valid_providers = ["openai", "anthropic", "ollama"];
    if !valid_providers.contains(&provider) {
        return Err(GlanceError::persistence(format!(
            "Invalid provider '{}'. Valid options: {}",
            provider,
            valid_providers.join(", ")
        )));
    }

    sqlx::query("UPDATE llm_settings SET provider = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(provider)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update provider: {e}")))?;

    Ok(())
}

/// Updates the LLM model.
pub async fn set_model(pool: &SqlitePool, model: &str) -> Result<()> {
    if model.is_empty() {
        return Err(GlanceError::persistence("Model name cannot be empty"));
    }

    sqlx::query("UPDATE llm_settings SET model = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(model)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update model: {e}")))?;

    Ok(())
}

/// Stores the API key for the current provider.
pub async fn set_api_key(
    pool: &SqlitePool,
    provider: &str,
    api_key: &str,
    secrets: &SecretStorage,
) -> Result<()> {
    let (storage, plaintext) = if secrets.is_secure() {
        let key = SecretStorage::llm_api_key(provider);
        secrets.store(&key, api_key)?;
        (ApiKeyStorage::Keyring, None)
    } else {
        (ApiKeyStorage::Plaintext, Some(api_key.to_string()))
    };

    sqlx::query(
        r#"
        UPDATE llm_settings 
        SET api_key_storage = ?, api_key_plaintext = ?, updated_at = datetime('now')
        WHERE id = 1
        "#,
    )
    .bind(storage.as_str())
    .bind(&plaintext)
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to store API key: {e}")))?;

    Ok(())
}

/// Retrieves the API key for the specified provider.
pub async fn get_api_key(
    pool: &SqlitePool,
    provider: &str,
    secrets: &SecretStorage,
) -> Result<Option<String>> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT api_key_storage, api_key_plaintext FROM llm_settings WHERE id = 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to get API key: {e}")))?;

    match row {
        Some((storage, plaintext)) => {
            let storage_type = ApiKeyStorage::from_str(&storage);
            match storage_type {
                ApiKeyStorage::None => Ok(None),
                ApiKeyStorage::Keyring => {
                    let key = SecretStorage::llm_api_key(provider);
                    secrets.retrieve(&key)
                }
                ApiKeyStorage::Plaintext => Ok(plaintext),
            }
        }
        None => Ok(None),
    }
}

/// Clears the stored API key.
pub async fn clear_api_key(
    pool: &SqlitePool,
    provider: &str,
    secrets: &SecretStorage,
) -> Result<()> {
    let key = SecretStorage::llm_api_key(provider);
    secrets.delete(&key)?;

    sqlx::query(
        r#"
        UPDATE llm_settings 
        SET api_key_storage = 'none', api_key_plaintext = NULL, updated_at = datetime('now')
        WHERE id = 1
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to clear API key: {e}")))?;

    Ok(())
}

/// Returns whether an API key is configured.
pub async fn has_api_key(pool: &SqlitePool) -> Result<bool> {
    let (storage,): (String,) =
        sqlx::query_as("SELECT api_key_storage FROM llm_settings WHERE id = 1")
            .fetch_one(pool)
            .await
            .map_err(|e| GlanceError::persistence(format!("Failed to check API key: {e}")))?;

    Ok(storage != "none")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_get_default_settings() {
        let pool = test_pool().await;

        let settings = get_llm_settings(&pool).await.unwrap();
        assert_eq!(settings.provider, "openai");
        assert_eq!(settings.model, "gpt-5");
        assert_eq!(settings.api_key_storage, ApiKeyStorage::None);
    }

    #[tokio::test]
    async fn test_set_provider() {
        let pool = test_pool().await;

        set_provider(&pool, "anthropic").await.unwrap();

        let settings = get_llm_settings(&pool).await.unwrap();
        assert_eq!(settings.provider, "anthropic");
    }

    #[tokio::test]
    async fn test_set_invalid_provider() {
        let pool = test_pool().await;

        let result = set_provider(&pool, "invalid").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid provider"));
    }

    #[tokio::test]
    async fn test_set_model() {
        let pool = test_pool().await;

        set_model(&pool, "claude-3-5-sonnet-latest").await.unwrap();

        let settings = get_llm_settings(&pool).await.unwrap();
        assert_eq!(settings.model, "claude-3-5-sonnet-latest");
    }

    #[tokio::test]
    async fn test_has_api_key() {
        let pool = test_pool().await;

        let has_key = has_api_key(&pool).await.unwrap();
        assert!(!has_key);
    }
}
