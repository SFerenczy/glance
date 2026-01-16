//! Anthropic LLM client implementation.
//!
//! Implements the LlmClient trait for Anthropic's API (Claude models).

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{GlanceError, Result};
use crate::llm::types::{Message, Role};
use crate::llm::LlmClient;

/// Default timeout for API requests.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Anthropic API base URL.
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Anthropic API version header.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Maximum tokens to generate.
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Anthropic client configuration.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Model to use (e.g., "claude-3-5-sonnet-latest").
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
}

impl AnthropicConfig {
    /// Creates a new config with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Sets the maximum tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

/// Anthropic LLM client.
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
    client: Client,
}

impl AnthropicClient {
    /// Creates a new Anthropic client with the given configuration.
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| GlanceError::llm(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Creates a client from environment variables.
    ///
    /// Reads `ANTHROPIC_API_KEY` for the API key.
    /// Optionally reads `ANTHROPIC_MODEL` for the model (defaults to "claude-3-5-sonnet-latest").
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| GlanceError::llm("ANTHROPIC_API_KEY environment variable not set"))?;

        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string());

        Self::new(AnthropicConfig::new(api_key, model))
    }

    /// Extracts the system message and converts remaining messages to Anthropic format.
    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    // Anthropic uses a separate system parameter
                    system = Some(msg.content.clone());
                }
                Role::User => {
                    converted.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: msg.content.clone(),
                    });
                }
                Role::Assistant => {
                    converted.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: msg.content.clone(),
                    });
                }
            }
        }

        (system, converted)
    }

    /// Parses an API error response.
    fn parse_error(status: reqwest::StatusCode, body: &str) -> GlanceError {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return GlanceError::llm("Authentication failed. Check your ANTHROPIC_API_KEY.");
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return GlanceError::llm("Rate limited. Please wait and try again.");
        }

        // Try to parse error message from response
        if let Ok(error_response) = serde_json::from_str::<AnthropicErrorResponse>(body) {
            return GlanceError::llm(format!(
                "Anthropic API error: {}",
                error_response.error.message
            ));
        }

        GlanceError::llm(format!("Anthropic API error ({}): {}", status, body))
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let (system, converted_messages) = Self::convert_messages(messages);

        let request = AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            system,
            messages: converted_messages,
            stream: false,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GlanceError::llm("Request timed out. Try again.")
                } else if e.is_connect() {
                    GlanceError::llm("Failed to connect to Anthropic API. Check your network.")
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
            return Err(Self::parse_error(status, &body));
        }

        let response: AnthropicResponse = serde_json::from_str(&body)
            .map_err(|e| GlanceError::llm(format!("Failed to parse response: {}", e)))?;

        // Extract text from content blocks
        let text = response
            .content
            .into_iter()
            .filter_map(|block| {
                if block.content_type == "text" {
                    Some(block.text)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            return Err(GlanceError::llm("No response from Anthropic"));
        }

        Ok(text)
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
    ) -> Result<BoxStream<'static, Result<String>>> {
        let (system, converted_messages) = Self::convert_messages(messages);

        let request = AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            system,
            messages: converted_messages,
            stream: true,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GlanceError::llm("Request timed out. Try again.")
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
            return Err(Self::parse_error(status, &body));
        }

        let stream = response.bytes_stream();

        let parsed_stream = stream
            .map(|chunk| {
                chunk
                    .map_err(|e| GlanceError::llm(format!("Stream error: {}", e)))
                    .and_then(|bytes| {
                        let text = String::from_utf8_lossy(&bytes);
                        parse_sse_chunk(&text)
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

/// Parses a Server-Sent Events chunk from the Anthropic streaming API.
fn parse_sse_chunk(chunk: &str) -> Result<Option<String>> {
    let mut content = String::new();

    for line in chunk.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                match event.event_type.as_str() {
                    "content_block_delta" => {
                        if let Some(delta) = event.delta {
                            if delta.delta_type == "text_delta" {
                                if let Some(text) = delta.text {
                                    content.push_str(&text);
                                }
                            }
                        }
                    }
                    "message_stop" => {
                        return Ok(if content.is_empty() {
                            None
                        } else {
                            Some(content)
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(if content.is_empty() {
        None
    } else {
        Some(content)
    })
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicError,
}

#[derive(Debug, Deserialize)]
struct AnthropicError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = AnthropicConfig::new("sk-ant-test", "claude-3-5-sonnet-latest");
        assert_eq!(config.api_key, "sk-ant-test");
        assert_eq!(config.model, "claude-3-5-sonnet-latest");
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(config.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn test_config_with_timeout() {
        let config =
            AnthropicConfig::new("sk-ant-test", "claude-3-5-sonnet-latest").with_timeout(60);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_config_with_max_tokens() {
        let config =
            AnthropicConfig::new("sk-ant-test", "claude-3-5-sonnet-latest").with_max_tokens(8192);
        assert_eq!(config.max_tokens, 8192);
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            Message::system("You are helpful."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let (system, converted) = AnthropicClient::convert_messages(&messages);

        assert_eq!(system, Some("You are helpful.".to_string()));
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    #[test]
    fn test_convert_messages_no_system() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi!")];

        let (system, converted) = AnthropicClient::convert_messages(&messages);

        assert_eq!(system, None);
        assert_eq!(converted.len(), 2);
    }

    #[test]
    fn test_parse_error_unauthorized() {
        let error = AnthropicClient::parse_error(reqwest::StatusCode::UNAUTHORIZED, "");
        assert!(error.to_string().contains("Authentication failed"));
    }

    #[test]
    fn test_parse_error_rate_limited() {
        let error = AnthropicClient::parse_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "");
        assert!(error.to_string().contains("Rate limited"));
    }

    #[test]
    fn test_parse_error_with_message() {
        let body = r#"{"error":{"message":"Invalid API key"}}"#;
        let error = AnthropicClient::parse_error(reqwest::StatusCode::BAD_REQUEST, body);
        assert!(error.to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_parse_sse_content_delta() {
        let chunk =
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#;
        let result = parse_sse_chunk(chunk).unwrap();
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_sse_message_stop() {
        let chunk = r#"data: {"type":"message_stop"}"#;
        let result = parse_sse_chunk(chunk).unwrap();
        assert_eq!(result, None);
    }
}
