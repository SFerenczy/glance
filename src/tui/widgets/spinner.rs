//! Spinner and loading indicator widgets for the TUI.
//!
//! Provides animated indicators for async operations.

#![allow(dead_code)] // Used by App spinner methods

use std::time::Instant;

/// Braille spinner frames for query execution.
const BRAILLE_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Dot animation frames for LLM thinking.
const DOT_FRAMES: &[&str] = &["", ".", "..", "..."];

/// Animation speed in milliseconds per frame.
const FRAME_DURATION_MS: u128 = 100;

/// Type of spinner animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerType {
    /// Braille spinner for query execution.
    Braille,
    /// Animated dots for LLM thinking.
    Dots,
}

/// Spinner state for animated indicators.
#[derive(Debug, Clone)]
pub struct Spinner {
    /// Type of spinner animation.
    spinner_type: SpinnerType,
    /// When the spinner started.
    start_time: Instant,
    /// Label to display with the spinner.
    label: String,
}

impl Spinner {
    /// Creates a new spinner with the given type and label.
    pub fn new(spinner_type: SpinnerType, label: impl Into<String>) -> Self {
        Self {
            spinner_type,
            start_time: Instant::now(),
            label: label.into(),
        }
    }

    /// Creates a new LLM thinking spinner.
    pub fn thinking() -> Self {
        Self::new(SpinnerType::Dots, "Thinking")
    }

    /// Creates a new query execution spinner.
    pub fn executing() -> Self {
        Self::new(SpinnerType::Braille, "Executing")
    }

    /// Creates a spinner for slash commands (e.g., "Connecting", "Refreshing").
    pub fn command(label: impl Into<String>) -> Self {
        Self::new(SpinnerType::Dots, label)
    }

    /// Returns the current frame of the animation.
    pub fn frame(&self) -> &'static str {
        let elapsed_ms = self.start_time.elapsed().as_millis();
        let frame_index = (elapsed_ms / FRAME_DURATION_MS) as usize;

        match self.spinner_type {
            SpinnerType::Braille => BRAILLE_FRAMES[frame_index % BRAILLE_FRAMES.len()],
            SpinnerType::Dots => DOT_FRAMES[frame_index % DOT_FRAMES.len()],
        }
    }

    /// Returns the display string for the spinner.
    pub fn display(&self) -> String {
        match self.spinner_type {
            SpinnerType::Braille => format!("{} {}", self.frame(), self.label),
            SpinnerType::Dots => format!("{}{}", self.label, self.frame()),
        }
    }

    /// Returns the label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_thinking() {
        let spinner = Spinner::thinking();
        assert_eq!(spinner.label(), "Thinking");
        // Frame should be one of the dot frames
        let frame = spinner.frame();
        assert!(DOT_FRAMES.contains(&frame));
    }

    #[test]
    fn test_spinner_executing() {
        let spinner = Spinner::executing();
        assert_eq!(spinner.label(), "Executing");
        // Frame should be one of the braille frames
        let frame = spinner.frame();
        assert!(BRAILLE_FRAMES.contains(&frame));
    }

    #[test]
    fn test_spinner_display() {
        let spinner = Spinner::thinking();
        let display = spinner.display();
        assert!(display.starts_with("Thinking"));
    }
}
