//! Query detail modal widget.
//!
//! Displays full SQL and details for a selected query in a modal overlay.

use crate::tui::app::{QueryLogEntry, QueryStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

/// Modal widget for displaying query details.
pub struct QueryDetailModal<'a> {
    entry: &'a QueryLogEntry,
}

impl<'a> QueryDetailModal<'a> {
    /// Creates a new query detail modal.
    pub fn new(entry: &'a QueryLogEntry) -> Self {
        Self { entry }
    }

    /// Calculates the modal area centered in the given area.
    fn modal_area(area: Rect) -> Rect {
        // Modal takes 80% width and 60% height, centered
        let width = (area.width * 80 / 100).clamp(40, 80);
        let height = (area.height * 60 / 100).clamp(10, 20);

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }

    /// Formats the execution time for display.
    fn format_time(&self) -> String {
        let millis = self.entry.execution_time.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else {
            format!("{:.2}s", self.entry.execution_time.as_secs_f64())
        }
    }
}

impl Widget for QueryDetailModal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_area = Self::modal_area(area);

        // Clear the area behind the modal
        Clear.render(modal_area, buf);

        // Build the modal content
        let status_style = match self.entry.status {
            QueryStatus::Success => Style::default().fg(Color::Green),
            QueryStatus::Error => Style::default().fg(Color::Red),
            QueryStatus::Cancelled => Style::default().fg(Color::Yellow),
        };

        let status_text = match self.entry.status {
            QueryStatus::Success => "✓ Success",
            QueryStatus::Error => "✗ Error",
            QueryStatus::Cancelled => "○ Cancelled",
        };

        let title = " Query Details [Esc to close] ";

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title)
            .title_alignment(Alignment::Center);

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Split inner area: status line, SQL, optional error
        let has_error = self.entry.error.is_some();
        let constraints = if has_error {
            vec![
                Constraint::Length(2), // Status line
                Constraint::Min(3),    // SQL
                Constraint::Length(3), // Error
            ]
        } else {
            vec![
                Constraint::Length(2), // Status line
                Constraint::Min(3),    // SQL
            ]
        };

        let chunks = Layout::default().constraints(constraints).split(inner);

        // Status line with execution info
        let info_line = match self.entry.status {
            QueryStatus::Cancelled => format!("{} | {}", status_text, self.entry.relative_time()),
            _ => match self.entry.row_count {
                Some(rows) => format!(
                    "{} | {} | {} row{}",
                    status_text,
                    self.format_time(),
                    rows,
                    if rows == 1 { "" } else { "s" }
                ),
                None => format!("{} | {}", status_text, self.format_time()),
            },
        };

        let status_paragraph =
            Paragraph::new(Line::from(vec![Span::styled(info_line, status_style)]));
        status_paragraph.render(chunks[0], buf);

        // SQL content
        let sql_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" SQL ")
            .title_style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        let sql_paragraph = Paragraph::new(self.entry.sql.as_str())
            .block(sql_block)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        sql_paragraph.render(chunks[1], buf);

        // Error message if present
        if let Some(error) = &self.entry.error {
            let error_block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Red))
                .title(" Error ")
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));

            let error_paragraph = Paragraph::new(error.as_str())
                .block(error_block)
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true });
            error_paragraph.render(chunks[2], buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use std::time::Duration;

    #[test]
    fn test_modal_area_calculation() {
        let area = Rect::new(0, 0, 100, 50);
        let modal = QueryDetailModal::modal_area(area);

        // Should be centered
        assert!(modal.x > 0);
        assert!(modal.y > 0);
        assert!(modal.x + modal.width <= area.width);
        assert!(modal.y + modal.height <= area.height);
    }

    #[test]
    fn test_format_time_ms() {
        let entry = QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(42), 1);
        let modal = QueryDetailModal::new(&entry);
        assert_eq!(modal.format_time(), "42ms");
    }

    #[test]
    fn test_format_time_seconds() {
        let entry = QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(2500), 1);
        let modal = QueryDetailModal::new(&entry);
        assert_eq!(modal.format_time(), "2.50s");
    }

    #[test]
    fn test_cancelled_status_renders() {
        let entry = QueryLogEntry::cancelled_with_source(
            "SELECT 1".to_string(),
            crate::tui::app::QuerySource::Manual,
        );
        let modal = QueryDetailModal::new(&entry);
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        modal.render(area, &mut buf);

        let rendered: String = buf.content.iter().map(|cell| cell.symbol()).collect();
        assert!(rendered.contains("Cancelled"));
    }
}
