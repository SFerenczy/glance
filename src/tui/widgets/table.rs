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
    show_row_numbers: bool,
    highlighted: bool,
}

impl<'a> ResultTable<'a> {
    /// Creates a new result table widget.
    pub fn new(result: &'a QueryResult) -> Self {
        Self {
            result,
            show_row_numbers: false,
            highlighted: false,
        }
    }

    /// Sets whether to show row numbers.
    pub fn show_row_numbers(self, show: bool) -> Self {
        Self {
            show_row_numbers: show,
            ..self
        }
    }

    /// Sets whether this table should be highlighted.
    pub fn highlighted(self, highlighted: bool) -> Self {
        Self {
            highlighted,
            ..self
        }
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
            .map(|col| Self::header_text(col).len().max(MIN_COLUMN_WIDTH))
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

    /// Builds the header label for a column (name + type).
    fn header_text(col: &crate::db::ColumnInfo) -> String {
        format!("{}:{}", col.name, col.data_type)
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

        // Data rows (with optional row numbers), or "No results" message if empty
        if self.result.rows.is_empty() {
            // Show "No results" message with execution time per FR-10.2
            let prefix = if self.show_row_numbers { "    " } else { "" };
            lines.push(Line::from(Span::styled(
                format!("{}│ No results found.", prefix),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                format!(
                    "{}│ Query executed successfully in {}ms.",
                    prefix,
                    self.result.execution_time.as_millis()
                ),
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (row_num, row) in self.result.rows.iter().enumerate() {
                lines.push(self.render_data_row(row_num + 1, row, &adjusted_widths));
            }
        }

        // Bottom border
        lines.push(self.render_border(&adjusted_widths, '└', '┴', '┘'));

        // Footer with row count and execution time (only if there are results)
        if !self.result.rows.is_empty() {
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
        }

        lines
    }

    /// Renders a horizontal border line.
    fn render_border(&self, widths: &[usize], left: char, mid: char, right: char) -> Line<'a> {
        let mut border = String::new();
        // Add spacing for row number column (4 chars: "{:>3} ") if enabled
        if self.show_row_numbers {
            border.push_str("    ");
        }
        border.push(left);

        for (i, &width) in widths.iter().enumerate() {
            border.push_str(&"─".repeat(width + 2));
            if i < widths.len() - 1 {
                border.push(mid);
            }
        }

        border.push(right);

        let mut style = Style::default().fg(Color::DarkGray);
        if self.highlighted {
            style = style.bg(Color::Rgb(40, 40, 0));
        }
        Line::from(Span::styled(border, style))
    }

    /// Renders the header row with column names.
    fn render_header_row(&self, widths: &[usize]) -> Line<'a> {
        let mut spans = Vec::new();

        let highlight_bg = if self.highlighted {
            Some(Color::Rgb(40, 40, 0))
        } else {
            None
        };

        // Add header for row number column (matches "{:>3} " format in data rows) if enabled
        if self.show_row_numbers {
            let mut style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD);
            if let Some(bg) = highlight_bg {
                style = style.bg(bg);
            }
            spans.push(Span::styled("  # ", style));
        }

        let mut border_style = Style::default().fg(Color::DarkGray);
        if let Some(bg) = highlight_bg {
            border_style = border_style.bg(bg);
        }
        spans.push(Span::styled("│", border_style));

        for (i, col) in self.result.columns.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(MIN_COLUMN_WIDTH);
            let name = Self::truncate(&Self::header_text(col), width);
            let padded = format!(" {:width$} ", name, width = width);

            let mut style = Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
            if let Some(bg) = highlight_bg {
                style = style.bg(bg);
            }

            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│", border_style));
        }

        Line::from(spans)
    }

    /// Renders a data row with optional row number.
    fn render_data_row(&self, row_num: usize, row: &[Value], widths: &[usize]) -> Line<'a> {
        let mut spans = Vec::new();

        // Highlight background color if table is highlighted
        let highlight_bg = if self.highlighted {
            Some(Color::Rgb(40, 40, 0)) // Subtle yellow highlight
        } else {
            None
        };

        // Row number prefix (dimmed) if enabled
        if self.show_row_numbers {
            let row_num_str = format!("{:>3} ", row_num);
            let mut style = Style::default().fg(Color::DarkGray);
            if let Some(bg) = highlight_bg {
                style = style.bg(bg);
            }
            spans.push(Span::styled(row_num_str, style));
        }

        let mut border_style = Style::default().fg(Color::DarkGray);
        if let Some(bg) = highlight_bg {
            border_style = border_style.bg(bg);
        }
        spans.push(Span::styled("│", border_style));

        for (i, value) in row.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(MIN_COLUMN_WIDTH);
            let display = value.to_display_string();
            let truncated = Self::truncate(&display, width);
            let padded = format!(" {:width$} ", truncated, width = width);

            let mut style = if value.is_null() {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default()
            };
            if let Some(bg) = highlight_bg {
                style = style.bg(bg);
            }

            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│", border_style));
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

        // id column: max of "id:integer" (10) and "1" (1) -> 10
        // name column: max of "name:varchar" (12) and "Alice" (5) -> 12
        // email column: max of "email:varchar" (13) and "alice@test.com" (14) -> 14
        assert_eq!(widths.len(), 3);
        assert_eq!(widths[0], 10);
        assert_eq!(widths[1], 12);
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
    fn test_header_includes_types() {
        let result = sample_result();
        let table = ResultTable::new(&result);
        let lines = table.render_to_lines(80);

        let header_line = lines.get(1).expect("header row");
        let header_text: String = header_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();

        assert!(header_text.contains("id:integer"));
        assert!(header_text.contains("name:varchar"));
        assert!(header_text.contains("email:varchar"));
    }

    #[test]
    fn test_empty_result() {
        let result = QueryResult::new();
        let table = ResultTable::new(&result);
        let lines = table.render_to_lines(80);

        assert_eq!(lines.len(), 1);
    }
}
