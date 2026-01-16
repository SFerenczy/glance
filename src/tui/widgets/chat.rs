//! Chat panel widget for the TUI.
//!
//! Displays the conversation history and query results.

use super::table::ResultTable;
use crate::tui::app::ChatMessage;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Chat panel widget.
pub struct ChatPanel<'a> {
    messages: &'a [ChatMessage],
    scroll_offset: usize,
    focused: bool,
}

impl<'a> ChatPanel<'a> {
    /// Creates a new chat panel widget.
    pub fn new(messages: &'a [ChatMessage], scroll_offset: usize, focused: bool) -> Self {
        Self {
            messages,
            scroll_offset,
            focused,
        }
    }

    /// Renders all messages to a vector of lines.
    fn render_messages(&self, available_width: usize) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        for message in self.messages {
            // Add spacing between messages
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }

            match message {
                ChatMessage::User(text) => {
                    lines.extend(self.render_user_message(text));
                }
                ChatMessage::Assistant(text) => {
                    lines.extend(self.render_assistant_message(text));
                }
                ChatMessage::Result(result) => {
                    lines.extend(self.render_result_message(result, available_width));
                }
                ChatMessage::Error(text) => {
                    lines.extend(self.render_error_message(text));
                }
                ChatMessage::System(text) => {
                    lines.extend(self.render_system_message(text));
                }
            }
        }

        lines
    }

    /// Renders a user message.
    fn render_user_message(&self, text: &str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // Label
        lines.push(Line::from(Span::styled(
            "You:",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )));

        // Content
        for line in text.lines() {
            lines.push(Line::from(format!("  {}", line)));
        }

        lines
    }

    /// Renders an assistant message.
    fn render_assistant_message(&self, text: &str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // Label
        lines.push(Line::from(Span::styled(
            "Glance:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        // Content
        for line in text.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::White),
            )));
        }

        lines
    }

    /// Renders a query result as a table.
    fn render_result_message(
        &self,
        result: &crate::db::QueryResult,
        available_width: usize,
    ) -> Vec<Line<'a>> {
        let table = ResultTable::new(result);
        // Convert the owned lines to static lifetime by collecting into owned data
        table
            .render_to_lines(available_width.saturating_sub(2))
            .into_iter()
            .map(|line| {
                Line::from(
                    line.spans
                        .into_iter()
                        .map(|span| Span::styled(span.content.into_owned(), span.style))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    /// Renders an error message.
    fn render_error_message(&self, text: &str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // Label
        lines.push(Line::from(Span::styled(
            "Error:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));

        // Content
        for line in text.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::Red),
            )));
        }

        lines
    }

    /// Renders a system message.
    fn render_system_message(&self, text: &str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // Content (no label for system messages, just styled differently)
        for line in text.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        }

        lines
    }
}

impl Widget for ChatPanel<'_> {
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

        // Calculate inner area for content
        let inner_area = block.inner(area);
        let available_width = inner_area.width as usize;
        let available_height = inner_area.height as usize;

        // Render all messages to lines
        let all_lines = self.render_messages(available_width);
        let total_lines = all_lines.len();

        // Calculate scroll position
        // scroll_offset is lines from bottom, so we need to convert
        let max_scroll = total_lines.saturating_sub(available_height);
        let clamped_scroll = self.scroll_offset.min(max_scroll);
        let start_line = max_scroll.saturating_sub(clamped_scroll);

        // Get visible lines
        let visible_lines: Vec<Line> = all_lines
            .into_iter()
            .skip(start_line)
            .take(available_height)
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ColumnInfo, QueryResult, Value};
    use std::time::Duration;

    #[test]
    fn test_chat_panel_empty() {
        let messages: Vec<ChatMessage> = vec![];
        let panel = ChatPanel::new(&messages, 0, false);
        let lines = panel.render_messages(80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_chat_panel_user_message() {
        let messages = vec![ChatMessage::User("Hello".to_string())];
        let panel = ChatPanel::new(&messages, 0, false);
        let lines = panel.render_messages(80);

        // Should have label + content
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_chat_panel_multiline_message() {
        let messages = vec![ChatMessage::User("Line 1\nLine 2\nLine 3".to_string())];
        let panel = ChatPanel::new(&messages, 0, false);
        let lines = panel.render_messages(80);

        // Should have label + 3 content lines
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_chat_panel_with_result() {
        let result = QueryResult {
            columns: vec![ColumnInfo::new("id", "integer")],
            rows: vec![vec![Value::Int(1)]],
            execution_time: Duration::from_millis(10),
            row_count: 1,
        };
        let messages = vec![ChatMessage::Result(result)];
        let panel = ChatPanel::new(&messages, 0, false);
        let lines = panel.render_messages(80);

        // Should have table lines
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_chat_panel_multiple_messages() {
        let messages = vec![
            ChatMessage::User("Hello".to_string()),
            ChatMessage::Assistant("Hi there!".to_string()),
        ];
        let panel = ChatPanel::new(&messages, 0, false);
        let lines = panel.render_messages(80);

        // Should have lines for both messages plus spacing
        assert!(lines.len() >= 5); // 2 + 2 + 1 spacing
    }
}
