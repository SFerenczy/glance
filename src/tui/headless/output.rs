//! Output formatting for headless mode.
//!
//! Provides different output formats: text, JSON, and frames.

use super::{HeadlessResult, HeadlessState};
use ratatui::buffer::Buffer;
use serde::Serialize;

/// Output format for headless mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text output of the final screen.
    #[default]
    Text,
    /// JSON output with screen, state, and metadata.
    Json,
    /// Frame-by-frame output showing state after each event.
    Frames,
}

/// Renders a ratatui buffer to a string.
pub struct ScreenRenderer;

impl ScreenRenderer {
    /// Renders a buffer to a plain text string.
    pub fn render(buffer: &Buffer) -> String {
        let area = buffer.area;
        if area.height == 0 {
            return String::new();
        }

        let lines = (0..area.height)
            .map(|y| {
                let line = (0..area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<Vec<_>>()
                    .join("");
                line.trim_end_matches(' ').to_string()
            })
            .collect::<Vec<_>>();

        let trimmed_lines = lines
            .into_iter()
            .rev()
            .skip_while(|line| line.is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        let output_lines = if trimmed_lines.is_empty() {
            vec![String::new()]
        } else {
            trimmed_lines
        };

        format!("{}\n", output_lines.join("\n"))
    }
}

/// JSON output structure.
#[derive(Debug, Serialize)]
struct JsonOutput {
    screen: String,
    screen_lines: Vec<String>,
    events_executed: usize,
    duration_ms: u64,
    assertions: AssertionSummary,
    state: HeadlessState,
}

#[derive(Debug, Serialize)]
struct AssertionSummary {
    passed: usize,
    failed: usize,
}

/// Formats headless execution results.
pub struct HeadlessOutput {
    format: OutputFormat,
}

impl HeadlessOutput {
    /// Creates a new output formatter.
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Formats the result according to the configured format.
    pub fn format(&self, result: &HeadlessResult) -> String {
        match self.format {
            OutputFormat::Text => self.format_text(result),
            OutputFormat::Json => self.format_json(result),
            OutputFormat::Frames => self.format_frames(result),
        }
    }

    /// Formats as plain text.
    fn format_text(&self, result: &HeadlessResult) -> String {
        let assertions = if result.assertions_passed > 0 || result.assertions_failed > 0 {
            format!(
                " | Assertions: {} passed, {} failed",
                result.assertions_passed, result.assertions_failed
            )
        } else {
            String::new()
        };

        format!(
            "{}\nEvents: {} executed in {}ms{}\n",
            result.screen,
            result.events_executed,
            result.duration.as_millis(),
            assertions
        )
    }

    /// Formats as JSON.
    fn format_json(&self, result: &HeadlessResult) -> String {
        let json_output = JsonOutput {
            screen: result.screen.clone(),
            screen_lines: result.screen_lines.clone(),
            events_executed: result.events_executed,
            duration_ms: result.duration.as_millis() as u64,
            assertions: AssertionSummary {
                passed: result.assertions_passed,
                failed: result.assertions_failed,
            },
            state: result.state.clone(),
        };

        serde_json::to_string_pretty(&json_output)
            .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize: {}\"}}", e))
    }

    /// Formats as frame-by-frame output.
    fn format_frames(&self, result: &HeadlessResult) -> String {
        let frames_text = result
            .frames
            .iter()
            .map(|frame| {
                let event_desc = frame.event.as_deref().unwrap_or("initial");
                format!(
                    "=== FRAME {} ({}) ===\n{}\n\n",
                    frame.number, event_desc, frame.screen
                )
            })
            .collect::<Vec<_>>()
            .join("");

        let assertions = if result.assertions_passed > 0 || result.assertions_failed > 0 {
            format!(
                "Assertions: {} passed, {} failed\n",
                result.assertions_passed, result.assertions_failed
            )
        } else {
            String::new()
        };

        format!(
            "{}Total: {} frames, {} events executed in {}ms\n{}",
            frames_text,
            result.frames.len(),
            result.events_executed,
            result.duration.as_millis(),
            assertions
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::headless::Frame;
    use std::time::Duration;

    fn make_test_result() -> HeadlessResult {
        HeadlessResult {
            screen: "Test screen\nLine 2".to_string(),
            screen_lines: vec!["Test screen".to_string(), "Line 2".to_string()],
            events_executed: 3,
            duration: Duration::from_millis(150),
            assertions_passed: 2,
            assertions_failed: 0,
            state: HeadlessState {
                input_text: "hello".to_string(),
                focus: "Input".to_string(),
                is_processing: false,
                message_count: 1,
                running: true,
                sql_completion_visible: false,
                sql_completion_selected: 0,
                sql_completion_count: 0,
            },
            frames: vec![
                Frame {
                    number: 0,
                    event: None,
                    screen: "Initial".to_string(),
                },
                Frame {
                    number: 1,
                    event: Some("type:hello".to_string()),
                    screen: "After typing".to_string(),
                },
            ],
        }
    }

    #[test]
    fn test_text_output() {
        let result = make_test_result();
        let output = HeadlessOutput::new(OutputFormat::Text);
        let text = output.format(&result);

        assert!(text.contains("Test screen"));
        assert!(text.contains("Events: 3 executed"));
        assert!(text.contains("Assertions: 2 passed, 0 failed"));
    }

    #[test]
    fn test_json_output() {
        let result = make_test_result();
        let output = HeadlessOutput::new(OutputFormat::Json);
        let json = output.format(&result);

        // Parse to verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["events_executed"], 3);
        assert_eq!(parsed["state"]["input_text"], "hello");
        assert_eq!(parsed["assertions"]["passed"], 2);
    }

    #[test]
    fn test_frames_output() {
        let result = make_test_result();
        let output = HeadlessOutput::new(OutputFormat::Frames);
        let frames = output.format(&result);

        assert!(frames.contains("=== FRAME 0 (initial) ==="));
        assert!(frames.contains("=== FRAME 1 (type:hello) ==="));
        assert!(frames.contains("Total: 2 frames"));
    }
}
