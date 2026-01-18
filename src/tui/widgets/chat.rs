//! Chat panel widget for the TUI.
//!
//! Displays the conversation history and query results.

use super::table::ResultTable;
use crate::tui::app::{ChatMessage, TextSelection};
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
    has_new_messages: bool,
    text_selection: Option<&'a TextSelection>,
}

impl<'a> ChatPanel<'a> {
    /// Creates a new chat panel widget.
    pub fn new(
        messages: &'a [ChatMessage],
        scroll_offset: usize,
        focused: bool,
        has_new_messages: bool,
        text_selection: Option<&'a TextSelection>,
    ) -> Self {
        Self {
            messages,
            scroll_offset,
            focused,
            has_new_messages,
            text_selection,
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

    /// Renders the text selection with inverted colors.
    fn render_selection(&self, buf: &mut Buffer, area: Rect, selection: &TextSelection) {
        // Normalize selection (start should be before end)
        let (start, end) = if selection.start.0 < selection.end.0
            || (selection.start.0 == selection.end.0 && selection.start.1 <= selection.end.1)
        {
            (selection.start, selection.end)
        } else {
            (selection.end, selection.start)
        };

        // Selection style: inverted colors
        let selection_style = Style::default()
            .bg(Color::White)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD);

        // Iterate over the selected rows
        for row in start.0..=end.0 {
            // Skip rows outside the chat area
            if row < area.y || row >= area.y + area.height {
                continue;
            }

            // Determine column range for this row
            let col_start = if row == start.0 {
                start.1.max(area.x)
            } else {
                area.x
            };
            let col_end = if row == end.0 {
                end.1.min(area.x + area.width)
            } else {
                area.x + area.width
            };

            // Apply selection style to each cell in the range
            for col in col_start..col_end {
                if col >= area.x && col < area.x + area.width {
                    if let Some(cell) = buf.cell_mut((col, row)) {
                        cell.set_style(selection_style);
                    }
                }
            }
        }
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

        // Render text selection with inverted colors
        if let Some(selection) = self.text_selection {
            self.render_selection(buf, area, selection);
        }

        // Show "new messages" indicator if scrolled up and there are new messages
        if self.has_new_messages && self.scroll_offset > 0 {
            let indicator = "↓ New messages ↓";
            let x = area.x + (area.width.saturating_sub(indicator.len() as u16)) / 2;
            let y = area.y + area.height - 1;
            if y > area.y {
                buf.set_string(
                    x,
                    y,
                    indicator,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }
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
        let panel = ChatPanel::new(&messages, 0, false, false, None);
        let lines = panel.render_messages(80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_chat_panel_user_message() {
        let messages = vec![ChatMessage::User("Hello".to_string())];
        let panel = ChatPanel::new(&messages, 0, false, false, None);
        let lines = panel.render_messages(80);

        // Should have label + content
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_chat_panel_multiline_message() {
        let messages = vec![ChatMessage::User("Line 1\nLine 2\nLine 3".to_string())];
        let panel = ChatPanel::new(&messages, 0, false, false, None);
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
            total_rows: Some(1),
            was_truncated: false,
        };
        let messages = vec![ChatMessage::Result(result)];
        let panel = ChatPanel::new(&messages, 0, false, false, None);
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
        let panel = ChatPanel::new(&messages, 0, false, false, None);
        let lines = panel.render_messages(80);

        // Should have lines for both messages plus spacing
        assert!(lines.len() >= 5); // 2 + 2 + 1 spacing
    }
}
