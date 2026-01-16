//! Ollama LLM client implementation.
//!
//! Implements the LlmClient trait for local Ollama instances.
//! Used primarily for integration testing without API costs.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{GlanceError, Result};
use crate::llm::types::Message;
use crate::llm::LlmClient;

/// Default timeout for API requests.
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Default Ollama API URL.
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Ollama client configuration.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Base URL for the Ollama API.
    pub base_url: String,
    /// Model to use (e.g., "llama3.2:3b", "codellama").
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl OllamaConfig {
    /// Creates a new config with the given model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            model: model.into(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Sets the base URL.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self::new("llama3.2:3b")
    }
}

/// Ollama LLM client.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    config: OllamaConfig,
    client: Client,
}

impl OllamaClient {
    /// Creates a new Ollama client with the given configuration.
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| GlanceError::llm(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Creates a client from environment variables.
    ///
    /// Reads `OLLAMA_URL` for the base URL (defaults to http://localhost:11434).
    /// Reads `OLLAMA_MODEL` for the model (defaults to "llama3.2:3b").
    pub fn from_env() -> Result<Self> {
        let base_url =
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string());

        Self::new(OllamaConfig::new(model).with_url(base_url))
    }

    /// Checks if Ollama is available at the configured URL.
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.config.base_url);
        self.client.get(&url).send().await.is_ok()
    }

    /// Converts internal messages to Ollama API format.
    fn convert_messages(messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.as_str().to_string(),
                content: m.content.clone(),
            })
            .collect()
    }

    /// Returns the chat API endpoint URL.
    fn chat_url(&self) -> String {
        format!("{}/api/chat", self.config.base_url)
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let request = OllamaRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: false,
        };

        let response = self
            .client
            .post(self.chat_url())
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GlanceError::llm("Request timed out. Try again.")
                } else if e.is_connect() {
                    GlanceError::llm(
                        "Failed to connect to Ollama. Is it running? Try: ollama serve",
                    )
                } else {
                    GlanceError::llm(format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| GlanceError::llm(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(GlanceError::llm(format!(
                "Ollama API error ({}): {}",
                status, body
            )));
        }

        let response: OllamaResponse = serde_json::from_str(&body)
            .map_err(|e| GlanceError::llm(format!("Failed to parse response: {}", e)))?;

        Ok(response.message.content)
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
    ) -> Result<BoxStream<'static, Result<String>>> {
        let request = OllamaRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: true,
        };

        let response = self
            .client
            .post(self.chat_url())
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GlanceError::llm("Request timed out. Try again.")
                } else if e.is_connect() {
                    GlanceError::llm(
                        "Failed to connect to Ollama. Is it running? Try: ollama serve",
                    )
                } else {
                    GlanceError::llm(format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GlanceError::llm(format!(
                "Ollama API error ({}): {}",
                status, body
            )));
        }

        let stream = response.bytes_stream();

        let parsed_stream = stream
            .map(|chunk| {
                chunk
                    .map_err(|e| GlanceError::llm(format!("Stream error: {}", e)))
                    .and_then(|bytes| {
                        let text = String::from_utf8_lossy(&bytes);
                        parse_stream_chunk(&text)
                    })
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(content)) => Some(Ok(content)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(parsed_stream.boxed())
    }
}

/// Parses a streaming chunk from the Ollama API.
fn parse_stream_chunk(chunk: &str) -> Result<Option<String>> {
    let mut content = String::new();

    for line in chunk.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(event) = serde_json::from_str::<OllamaStreamEvent>(line) {
            content.push_str(&event.message.content);
        }
    }

    Ok(if content.is_empty() {
        None
    } else {
        Some(content)
    })
}

// Ollama API types

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamEvent {
    message: OllamaMessage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = OllamaConfig::new("llama3.2:3b");
        assert_eq!(config.model, "llama3.2:3b");
        assert_eq!(config.base_url, DEFAULT_OLLAMA_URL);
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_config_with_url() {
        let config = OllamaConfig::new("llama3.2:3b").with_url("http://custom:11434");
        assert_eq!(config.base_url, "http://custom:11434");
    }

    #[test]
    fn test_config_with_timeout() {
        let config = OllamaConfig::new("llama3.2:3b").with_timeout(120);
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_config_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.model, "llama3.2:3b");
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            Message::system("You are helpful."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let converted = OllamaClient::convert_messages(&messages);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }

    #[test]
    fn test_chat_url() {
        let config = OllamaConfig::new("llama3.2:3b");
        let client = OllamaClient::new(config).unwrap();
        assert_eq!(client.chat_url(), "http://localhost:11434/api/chat");
    }

    #[test]
    fn test_parse_stream_chunk() {
        let chunk = r#"{"message":{"role":"assistant","content":"Hello"}}"#;
        let result = parse_stream_chunk(chunk).unwrap();
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_stream_chunk_empty() {
        let result = parse_stream_chunk("").unwrap();
        assert_eq!(result, None);
    }
}
