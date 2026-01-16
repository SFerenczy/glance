//! Input widget for the TUI.
//!
//! Provides a text input field with cursor support.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Input bar widget.
#[allow(dead_code)] // Used for cursor positioning in ui.rs
pub struct InputBar<'a> {
    text: &'a str,
    cursor: usize,
    focused: bool,
}

impl<'a> InputBar<'a> {
    /// Creates a new input bar widget.
    pub fn new(text: &'a str, cursor: usize, focused: bool) -> Self {
        Self {
            text,
            cursor,
            focused,
        }
    }
}

impl Widget for InputBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Input ");

        // Build the input line with prompt
        let prompt_style = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);
        let text_style = Style::default();

        let line = Line::from(vec![
            Span::styled("> ", prompt_style),
            Span::styled(self.text, text_style),
        ]);

        let paragraph = Paragraph::new(line).block(block);

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_bar_creation() {
        let input = InputBar::new("hello", 5, true);
        assert_eq!(input.text, "hello");
        assert_eq!(input.cursor, 5);
        assert!(input.focused);
    }
}
