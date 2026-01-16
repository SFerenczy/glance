//! Result table widget for the TUI.
//!
//! Renders query results as formatted tables with column headers,
//! auto-sized columns, and styled NULL values.

use crate::db::{QueryResult, Value};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Maximum width for any column.
const MAX_COLUMN_WIDTH: usize = 40;

/// Minimum width for any column.
const MIN_COLUMN_WIDTH: usize = 4;

/// Widget for rendering a query result as a table.
pub struct ResultTable<'a> {
    result: &'a QueryResult,
}

impl<'a> ResultTable<'a> {
    /// Creates a new result table widget.
    pub fn new(result: &'a QueryResult) -> Self {
        Self { result }
    }

    /// Calculates the optimal width for each column.
    fn calculate_column_widths(&self) -> Vec<usize> {
        if self.result.columns.is_empty() {
            return vec![];
        }

        let mut widths: Vec<usize> = self
            .result
            .columns
            .iter()
            .map(|col| col.name.len().max(MIN_COLUMN_WIDTH))
            .collect();

        for row in &self.result.rows {
            for (i, value) in row.iter().enumerate() {
                if i < widths.len() {
                    let value_len = value.to_display_string().len();
                    widths[i] = widths[i].max(value_len);
                }
            }
        }

        // Cap at max width
        widths.iter().map(|&w| w.min(MAX_COLUMN_WIDTH)).collect()
    }

    /// Truncates a string to fit within the given width, adding ellipsis if needed.
    fn truncate(s: &str, max_width: usize) -> String {
        if s.len() <= max_width {
            s.to_string()
        } else if max_width <= 3 {
            s.chars().take(max_width).collect()
        } else {
            format!("{}...", &s[..max_width - 3])
        }
    }

    /// Renders the table to a vector of Lines for embedding in other widgets.
    pub fn render_to_lines(&self, available_width: usize) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        if self.result.columns.is_empty() {
            lines.push(Line::from(Span::styled(
                "(empty result)",
                Style::default().fg(Color::DarkGray),
            )));
            return lines;
        }

        let widths = self.calculate_column_widths();

        // Calculate total table width and adjust if needed
        let total_width: usize = widths.iter().sum::<usize>() + widths.len() * 3 + 1; // borders and padding
        let scale_factor = if total_width > available_width && available_width > 0 {
            available_width as f64 / total_width as f64
        } else {
            1.0
        };

        let adjusted_widths: Vec<usize> = widths
            .iter()
            .map(|&w| ((w as f64 * scale_factor) as usize).max(MIN_COLUMN_WIDTH))
            .collect();

        // Top border
        lines.push(self.render_border(&adjusted_widths, '┌', '┬', '┐'));

        // Header row
        lines.push(self.render_header_row(&adjusted_widths));

        // Header separator
        lines.push(self.render_border(&adjusted_widths, '├', '┼', '┤'));

        // Data rows
        for row in &self.result.rows {
            lines.push(self.render_data_row(row, &adjusted_widths));
        }

        // Bottom border
        lines.push(self.render_border(&adjusted_widths, '└', '┴', '┘'));

        // Footer with row count and execution time
        let footer = format!(
            "{} row{} returned ({}ms)",
            self.result.row_count,
            if self.result.row_count == 1 { "" } else { "s" },
            self.result.execution_time.as_millis()
        );
        lines.push(Line::from(Span::styled(
            footer,
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    /// Renders a horizontal border line.
    fn render_border(&self, widths: &[usize], left: char, mid: char, right: char) -> Line<'a> {
        let mut border = String::new();
        border.push(left);

        for (i, &width) in widths.iter().enumerate() {
            border.push_str(&"─".repeat(width + 2));
            if i < widths.len() - 1 {
                border.push(mid);
            }
        }

        border.push(right);

        Line::from(Span::styled(border, Style::default().fg(Color::DarkGray)))
    }

    /// Renders the header row with column names.
    fn render_header_row(&self, widths: &[usize]) -> Line<'a> {
        let mut spans = Vec::new();
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

        for (i, col) in self.result.columns.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(MIN_COLUMN_WIDTH);
            let name = Self::truncate(&col.name, width);
            let padded = format!(" {:width$} ", name, width = width);

            spans.push(Span::styled(
                padded,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }

        Line::from(spans)
    }

    /// Renders a data row.
    fn render_data_row(&self, row: &[Value], widths: &[usize]) -> Line<'a> {
        let mut spans = Vec::new();
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

        for (i, value) in row.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(MIN_COLUMN_WIDTH);
            let display = value.to_display_string();
            let truncated = Self::truncate(&display, width);
            let padded = format!(" {:width$} ", truncated, width = width);

            let style = if value.is_null() {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default()
            };

            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }

        Line::from(spans)
    }
}

impl Widget for ResultTable<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.render_to_lines(area.width as usize);

        for (i, line) in lines.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }
            let y = area.y + i as u16;
            buf.set_line(area.x, y, line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ColumnInfo;
    use std::time::Duration;

    fn sample_result() -> QueryResult {
        QueryResult {
            columns: vec![
                ColumnInfo::new("id", "integer"),
                ColumnInfo::new("name", "varchar"),
                ColumnInfo::new("email", "varchar"),
            ],
            rows: vec![
                vec![
                    Value::Int(1),
                    Value::String("Alice".to_string()),
                    Value::String("alice@test.com".to_string()),
                ],
                vec![Value::Int(2), Value::String("Bob".to_string()), Value::Null],
            ],
            execution_time: Duration::from_millis(23),
            row_count: 2,
            total_rows: Some(2),
            was_truncated: false,
        }
    }

    #[test]
    fn test_calculate_column_widths() {
        let result = sample_result();
        let table = ResultTable::new(&result);
        let widths = table.calculate_column_widths();

        // id column: max of "id" (2) and "1" (1) -> MIN_COLUMN_WIDTH (4)
        // name column: max of "name" (4) and "Alice" (5) -> 5
        // email column: max of "email" (5) and "alice@test.com" (14) -> 14
        assert_eq!(widths.len(), 3);
        assert_eq!(widths[0], 4); // MIN_COLUMN_WIDTH
        assert_eq!(widths[1], 5);
        assert_eq!(widths[2], 14);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(ResultTable::truncate("hello", 10), "hello");
        assert_eq!(ResultTable::truncate("hello world", 8), "hello...");
        assert_eq!(ResultTable::truncate("hi", 2), "hi");
        assert_eq!(ResultTable::truncate("hello", 3), "hel");
    }

    #[test]
    fn test_render_to_lines() {
        let result = sample_result();
        let table = ResultTable::new(&result);
        let lines = table.render_to_lines(80);

        // Should have: top border, header, separator, 2 data rows, bottom border, footer
        assert_eq!(lines.len(), 7);
    }

    #[test]
    fn test_empty_result() {
        let result = QueryResult::new();
        let table = ResultTable::new(&result);
        let lines = table.render_to_lines(80);

        assert_eq!(lines.len(), 1);
    }
}
