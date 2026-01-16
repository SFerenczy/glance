//! Sidebar widget for the TUI.
//!
//! Displays the query log with executed SQL queries.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Sidebar widget for query log.
pub struct Sidebar {
    focused: bool,
}

impl Sidebar {
    /// Creates a new sidebar widget.
    pub fn new(focused: bool) -> Self {
        Self { focused }
    }
}

impl Widget for Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Query Log ");

        // Placeholder content for Phase 5
        let placeholder = vec![Line::from(Span::styled(
            "No queries yet",
            Style::default().fg(Color::DarkGray),
        ))];

        let paragraph = Paragraph::new(placeholder).block(block);

        paragraph.render(area, buf);
    }
}
