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

/// Calculates the scroll offset needed to keep the cursor visible.
///
/// Returns the number of characters to skip from the start of the text.
pub fn calculate_scroll_offset(cursor: usize, _text_len: usize, available_width: usize) -> usize {
    if cursor <= available_width {
        0
    } else {
        cursor.saturating_sub(available_width)
    }
}

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

        // Calculate available width for text (subtract borders and prompt)
        // Border left (1) + prompt "> " (2) + border right (1) + cursor space (1) = 5
        let available_width = area.width.saturating_sub(5) as usize;
        let scroll_offset = calculate_scroll_offset(self.cursor, self.text.len(), available_width);

        // Get the visible portion of text
        let visible_text = if scroll_offset < self.text.len() {
            &self.text[scroll_offset..]
        } else {
            ""
        };

        let line = Line::from(vec![
            Span::styled("> ", prompt_style),
            Span::styled(visible_text, text_style),
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

    #[test]
    fn test_scroll_offset_cursor_within_width() {
        // Cursor at position 5, width 20 -> no scroll needed
        assert_eq!(calculate_scroll_offset(5, 10, 20), 0);
        // Cursor at position 20, width 20 -> no scroll needed
        assert_eq!(calculate_scroll_offset(20, 30, 20), 0);
    }

    #[test]
    fn test_scroll_offset_cursor_beyond_width() {
        // Cursor at position 25, width 20 -> scroll by 5
        assert_eq!(calculate_scroll_offset(25, 30, 20), 5);
        // Cursor at position 50, width 20 -> scroll by 30
        assert_eq!(calculate_scroll_offset(50, 60, 20), 30);
    }

    #[test]
    fn test_scroll_offset_edge_cases() {
        // Cursor at 0 -> no scroll
        assert_eq!(calculate_scroll_offset(0, 10, 20), 0);
        // Width is 0 -> cursor position becomes offset
        assert_eq!(calculate_scroll_offset(5, 10, 0), 5);
    }
}
