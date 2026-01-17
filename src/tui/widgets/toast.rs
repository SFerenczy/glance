//! Toast notification widget for the TUI.
//!
//! Displays temporary messages that auto-dismiss.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Toast notification widget.
pub struct Toast<'a> {
    message: &'a str,
}

impl<'a> Toast<'a> {
    /// Creates a new toast widget.
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }

    /// Calculates the area for the toast (bottom-right corner).
    pub fn area(screen: Rect) -> Rect {
        let width = 40.min(screen.width.saturating_sub(4));
        let height = 3;
        let x = screen.width.saturating_sub(width + 2);
        let y = screen.height.saturating_sub(height + 1);
        Rect::new(x, y, width, height)
    }
}

impl Widget for Toast<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        // Truncate message if needed
        let max_len = inner.width as usize;
        let display_msg = if self.message.len() > max_len {
            format!("{}…", &self.message[..max_len.saturating_sub(1)])
        } else {
            self.message.to_string()
        };

        let line = Line::from(vec![Span::styled(
            display_msg,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]);

        let paragraph = Paragraph::new(line);
        paragraph.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_area() {
        let screen = Rect::new(0, 0, 80, 24);
        let area = Toast::area(screen);
        assert!(area.x > 0);
        assert!(area.y > 0);
        assert_eq!(area.height, 3);
    }
}
