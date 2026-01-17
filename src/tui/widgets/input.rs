//! Input widget for the TUI.
//!
//! Provides a text input field with cursor support and mode indicator.

use crate::tui::app::InputMode;
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
    mode: InputMode,
    vim_mode_enabled: bool,
}

impl<'a> InputBar<'a> {
    /// Creates a new input bar widget.
    pub fn new(
        text: &'a str,
        cursor: usize,
        focused: bool,
        mode: InputMode,
        vim_mode_enabled: bool,
    ) -> Self {
        Self {
            text,
            cursor,
            focused,
            mode,
            vim_mode_enabled,
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

        // Mode indicator styling
        let mode_style = match self.mode {
            InputMode::Normal => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            InputMode::Insert => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Input ");

        // Build the input line with prompt and mode indicator
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

        // Render mode indicator on the right side of the input area (only when vim mode is enabled)
        if self.focused && self.vim_mode_enabled && area.width > 20 {
            let mode_text = self.mode.indicator();
            let mode_x = area.x + area.width - mode_text.len() as u16 - 2;
            let mode_y = area.y + 1;
            if mode_x > area.x + 3 {
                buf.set_string(mode_x, mode_y, mode_text, mode_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_bar_creation() {
        let input = InputBar::new("hello", 5, true, InputMode::Insert, false);
        assert_eq!(input.text, "hello");
        assert_eq!(input.cursor, 5);
        assert!(input.focused);
        assert!(!input.vim_mode_enabled);
    }

    #[test]
    fn test_input_bar_with_vim_mode() {
        let input = InputBar::new("test", 2, true, InputMode::Normal, true);
        assert!(input.vim_mode_enabled);
    }
}
