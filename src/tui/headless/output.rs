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
        let mut output = String::new();

        for y in 0..area.height {
            for x in 0..area.width {
                let cell = buffer.cell((x, y)).unwrap();
                output.push_str(cell.symbol());
            }
            // Trim trailing whitespace from each line
            while output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
        }

        // Remove trailing empty lines
        while output.ends_with("\n\n") {
            output.pop();
        }

        output
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
        let mut output = String::new();

        // Screen content
        output.push_str(&result.screen);
        output.push('\n');

        // Summary line
        output.push_str(&format!(
            "Events: {} executed in {}ms",
            result.events_executed,
            result.duration.as_millis()
        ));

        if result.assertions_passed > 0 || result.assertions_failed > 0 {
            output.push_str(&format!(
                " | Assertions: {} passed, {} failed",
                result.assertions_passed, result.assertions_failed
            ));
        }

        output.push('\n');
        output
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
        let mut output = String::new();

        for frame in &result.frames {
            let event_desc = match &frame.event {
                Some(e) => e.clone(),
                None => "initial".to_string(),
            };

            output.push_str(&format!(
                "=== FRAME {} ({}) ===\n",
                frame.number, event_desc
            ));
            output.push_str(&frame.screen);
            output.push_str("\n\n");
        }

        // Final summary
        output.push_str(&format!(
            "Total: {} frames, {} events executed in {}ms\n",
            result.frames.len(),
            result.events_executed,
            result.duration.as_millis()
        ));

        if result.assertions_passed > 0 || result.assertions_failed > 0 {
            output.push_str(&format!(
                "Assertions: {} passed, {} failed\n",
                result.assertions_passed, result.assertions_failed
            ));
        }

        output
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
