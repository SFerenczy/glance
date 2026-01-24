//! OpenAI LLM client implementation.
//!
//! Implements the LlmClient trait for OpenAI's API (GPT-4, etc.).

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use crate::error::{GlanceError, Result};
use crate::llm::tools::ToolDefinition;
use crate::llm::types::{LlmResponse, Message, ToolCall, ToolResult};
use crate::llm::LlmClient;

/// Default timeout for API requests.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// OpenAI API base URL.
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Maximum number of retry attempts for transient errors.
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Base delay for exponential backoff (milliseconds).
const RETRY_BASE_DELAY_MS: u64 = 1000;

/// OpenAI client configuration.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Model to use (e.g., "gpt-5", "gpt-5-mini").
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl OpenAiConfig {
    /// Creates a new config with the given API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

/// OpenAI LLM client.
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    config: OpenAiConfig,
    client: Client,
}

impl OpenAiClient {
    /// Creates a new OpenAI client with the given configuration.
    pub fn new(config: OpenAiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| GlanceError::llm(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Creates a client from environment variables.
    ///
    /// Reads `OPENAI_API_KEY` for the API key.
    /// Optionally reads `OPENAI_MODEL` for the model (defaults to "gpt-5").
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| GlanceError::llm("OPENAI_API_KEY environment variable not set"))?;

        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5".to_string());

        Self::new(OpenAiConfig::new(api_key, model))
    }

    /// Converts internal messages to OpenAI API format.
    fn convert_messages(messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .map(|m| OpenAiMessage {
                role: m.role.as_str().to_string(),
                content: Some(m.content.as_ref().to_string()),
                tool_calls: None,
                tool_call_id: None,
            })
            .collect()
    }

    /// Converts tool definitions to OpenAI API format.
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<OpenAiTool> {
        tools
            .iter()
            .map(|t| OpenAiTool {
                tool_type: "function".to_string(),
                function: OpenAiFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect()
    }

    /// Converts OpenAI tool calls to our internal format.
    fn convert_tool_calls(tool_calls: &[OpenAiToolCall]) -> Vec<ToolCall> {
        tool_calls
            .iter()
            .map(|tc| ToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            })
            .collect()
    }

    /// Parses an API error response and returns (error, is_retryable).
    fn parse_error(status: reqwest::StatusCode, body: &str) -> (GlanceError, bool) {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return (
                GlanceError::llm("Authentication failed. Check your OPENAI_API_KEY."),
                false,
            );
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return (
                GlanceError::llm("Rate limited. Please wait and try again."),
                true, // Rate limits are retryable
            );
        }

        // 5xx errors are generally retryable
        let is_retryable = status.is_server_error();

        // Try to parse error message from response
        if let Ok(error_response) = serde_json::from_str::<OpenAiErrorResponse>(body) {
            return (
                GlanceError::llm(format!(
                    "OpenAI API error: {}",
                    error_response.error.message
                )),
                is_retryable,
            );
        }

        (
            GlanceError::llm(format!("OpenAI API error ({}): {}", status, body)),
            is_retryable,
        )
    }

    /// Determines if a request error is retryable.
    fn is_retryable_request_error(error: &reqwest::Error) -> bool {
        error.is_timeout() || error.is_connect()
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let response = self.complete_with_tools(messages, &[]).await?;
        Ok(response.content)
    }

    async fn complete_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let request = OpenAiRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: false,
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
        };

        let mut last_error = None;
        let mut delay = Duration::from_millis(RETRY_BASE_DELAY_MS);

        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            debug!(
                "OpenAI API request attempt {} of {}",
                attempt, MAX_RETRY_ATTEMPTS
            );

            let result = self
                .client
                .post(OPENAI_API_URL)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .map_err(|e| GlanceError::llm(format!("Failed to read response: {}", e)))?;

                    if status.is_success() {
                        let response: OpenAiResponse =
                            serde_json::from_str(&body).map_err(|e| {
                                GlanceError::llm(format!("Failed to parse response: {}", e))
                            })?;

                        let choice = response
                            .choices
                            .into_iter()
                            .next()
                            .ok_or_else(|| GlanceError::llm("No response from OpenAI"))?;

                        let content = choice.message.content.unwrap_or_default();
                        let tool_calls = choice
                            .message
                            .tool_calls
                            .map(|tcs| Self::convert_tool_calls(&tcs))
                            .unwrap_or_default();

                        return Ok(LlmResponse::with_tool_calls(content, tool_calls));
                    }

                    let (error, is_retryable) = Self::parse_error(status, &body);
                    last_error = Some(error);

                    if !is_retryable || attempt >= MAX_RETRY_ATTEMPTS {
                        break;
                    }

                    warn!(
                        "OpenAI API request failed (attempt {}), retrying in {:?}: {}",
                        attempt, delay, status
                    );
                }
                Err(e) => {
                    let is_retryable = Self::is_retryable_request_error(&e);
                    let error = if e.is_timeout() {
                        GlanceError::llm("Request timed out. Try again.")
                    } else if e.is_connect() {
                        GlanceError::llm("Failed to connect to OpenAI API. Check your network.")
                    } else {
                        GlanceError::llm(format!("Request failed: {}", e))
                    };
                    last_error = Some(error);

                    if !is_retryable || attempt >= MAX_RETRY_ATTEMPTS {
                        break;
                    }

                    warn!(
                        "OpenAI API request failed (attempt {}), retrying in {:?}",
                        attempt, delay
                    );
                }
            }

            tokio::time::sleep(delay).await;
            delay *= 2; // Exponential backoff
        }

        Err(last_error.expect("at least one attempt was made"))
    }

    async fn continue_with_tool_results(
        &self,
        messages: &[Message],
        assistant_tool_calls: &[ToolCall],
        tool_results: &[ToolResult],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let mut api_messages = Self::convert_messages(messages);

        // Add the assistant message with tool_calls (required by OpenAI)
        if !assistant_tool_calls.is_empty() {
            api_messages.push(OpenAiMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(
                    assistant_tool_calls
                        .iter()
                        .map(|tc| OpenAiToolCall {
                            id: tc.id.clone(),
                            call_type: "function".to_string(),
                            function: OpenAiToolCallFunction {
                                name: tc.name.clone(),
                                arguments: tc.arguments.clone(),
                            },
                        })
                        .collect(),
                ),
                tool_call_id: None,
            });
        }

        // Add tool result messages
        for result in tool_results {
            api_messages.push(OpenAiMessage {
                role: "tool".to_string(),
                content: Some(result.content.clone()),
                tool_calls: None,
                tool_call_id: Some(result.tool_call_id.clone()),
            });
        }

        let request = OpenAiRequest {
            model: self.config.model.clone(),
            messages: api_messages,
            stream: false,
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
        };

        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| GlanceError::llm(format!("Request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| GlanceError::llm(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            let (error, _) = Self::parse_error(status, &body);
            return Err(error);
        }

        let response: OpenAiResponse = serde_json::from_str(&body)
            .map_err(|e| GlanceError::llm(format!("Failed to parse response: {}", e)))?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| GlanceError::llm("No response from OpenAI"))?;

        let content = choice.message.content.unwrap_or_default();
        let tool_calls = choice
            .message
            .tool_calls
            .map(|tcs| Self::convert_tool_calls(&tcs))
            .unwrap_or_default();

        Ok(LlmResponse::with_tool_calls(content, tool_calls))
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
    ) -> Result<BoxStream<'static, Result<String>>> {
        let request = OpenAiRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: true,
            tools: None,
        };

        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
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
            let (error, _) = Self::parse_error(status, &body);
            return Err(error);
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

/// Parses a Server-Sent Events chunk from the OpenAI streaming API.
fn parse_sse_chunk(chunk: &str) -> Result<Option<String>> {
    let mut content = String::new();

    for line in chunk.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if line == "data: [DONE]" {
            return Ok(if content.is_empty() {
                None
            } else {
                Some(content)
            });
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<OpenAiStreamEvent>(data) {
                if let Some(choice) = event.choices.first() {
                    if let Some(ref delta_content) = choice.delta.content {
                        content.push_str(delta_content);
                    }
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

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamEvent {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiDelta,
}

#[derive(Debug, Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiError,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = OpenAiConfig::new("sk-test", "gpt-5");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-5");
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_config_with_timeout() {
        let config = OpenAiConfig::new("sk-test", "gpt-5").with_timeout(60);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            Message::system("You are helpful."),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let converted = OpenAiClient::convert_messages(&messages);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }

    #[test]
    fn test_parse_sse_chunk() {
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}

"#;
        let result = parse_sse_chunk(chunk).unwrap();
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_sse_done() {
        let chunk = "data: [DONE]\n";
        let result = parse_sse_chunk(chunk).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_error_unauthorized() {
        let (error, is_retryable) =
            OpenAiClient::parse_error(reqwest::StatusCode::UNAUTHORIZED, "");
        assert!(error.to_string().contains("Authentication failed"));
        assert!(!is_retryable);
    }

    #[test]
    fn test_parse_error_rate_limited() {
        let (error, is_retryable) =
            OpenAiClient::parse_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "");
        assert!(error.to_string().contains("Rate limited"));
        assert!(is_retryable);
    }

    #[test]
    fn test_parse_error_with_message() {
        let body = r#"{"error":{"message":"Invalid API key"}}"#;
        let (error, _) = OpenAiClient::parse_error(reqwest::StatusCode::BAD_REQUEST, body);
        assert!(error.to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_parse_error_server_error_is_retryable() {
        let (_, is_retryable) =
            OpenAiClient::parse_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "");
        assert!(is_retryable);
    }
}
