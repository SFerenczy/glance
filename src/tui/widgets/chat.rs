//! Chat panel widget for the TUI.
//!
//! Displays the conversation history and query results.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Chat panel widget.
pub struct ChatPanel {
    focused: bool,
}

impl ChatPanel {
    /// Creates a new chat panel widget.
    pub fn new(focused: bool) -> Self {
        Self { focused }
    }
}

impl Widget for ChatPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Chat ");

        // Placeholder content for Phase 5
        let placeholder = vec![
            Line::from(Span::styled(
                "Welcome to Glance!",
                Style::default().fg(Color::Green),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Ask questions about your database in natural language.",
                Style::default().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Commands:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                "  /sql <query>  - Execute raw SQL",
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  /schema       - Show database schema",
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  /clear        - Clear chat history",
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  /help         - Show help",
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  /quit         - Exit application",
                Style::default().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Tab to switch focus, Ctrl+C to exit.",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(placeholder).block(block);

        paragraph.render(area, buf);
    }
}
