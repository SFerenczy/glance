//! Sidebar widget for the TUI.
//!
//! Displays the query log with executed SQL queries.

use crate::tui::app::{QueryLogEntry, QuerySource, QueryStatus};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

/// Sidebar widget for query log.
pub struct Sidebar<'a> {
    entries: &'a [QueryLogEntry],
    selected: Option<usize>,
    focused: bool,
}

impl<'a> Sidebar<'a> {
    /// Creates a new sidebar widget.
    pub fn new(entries: &'a [QueryLogEntry], selected: Option<usize>, focused: bool) -> Self {
        Self {
            entries,
            selected,
            focused,
        }
    }

    /// Formats execution time for display.
    fn format_time(entry: &QueryLogEntry) -> String {
        let millis = entry.execution_time.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else {
            format!("{:.1}s", entry.execution_time.as_secs_f64())
        }
    }

    /// Returns a source indicator character.
    fn source_indicator(source: QuerySource) -> &'static str {
        match source {
            QuerySource::Manual => "⌨",    // Keyboard icon for manual
            QuerySource::Generated => "✎", // Pencil for LLM-generated (confirmed)
            QuerySource::Auto => "⚡",     // Lightning for auto-executed
        }
    }

    /// Creates a list item for a query entry.
    #[allow(dead_code)]
    fn make_list_item(entry: &QueryLogEntry, width: usize) -> ListItem<'static> {
        Self::make_list_item_with_separator(entry, width, false)
    }

    /// Creates a list item for a query entry, optionally with a separator line above.
    fn make_list_item_with_separator(
        entry: &QueryLogEntry,
        width: usize,
        with_separator: bool,
    ) -> ListItem<'static> {
        // Calculate available width for SQL preview
        // Account for: "▸ " (2) + status icon (2) + time + rows
        let preview_width = width.saturating_sub(4).min(30);

        let status_icon = match entry.status {
            QueryStatus::Success => Span::styled("✓ ", Style::default().fg(Color::Green)),
            QueryStatus::Error => Span::styled("✗ ", Style::default().fg(Color::Red)),
            QueryStatus::Cancelled => Span::styled("○ ", Style::default().fg(Color::Yellow)),
        };

        // Source indicator with color
        let source_icon = Span::styled(
            format!("{} ", Self::source_indicator(entry.source)),
            Style::default().fg(match entry.source {
                QuerySource::Manual => Color::Blue,
                QuerySource::Generated => Color::Yellow,
                QuerySource::Auto => Color::Magenta,
            }),
        );

        let sql_preview = entry.sql_preview(preview_width);
        let sql_span = Span::styled(
            format!("▸ {}...", sql_preview),
            Style::default().fg(Color::White),
        );

        let time_str = Self::format_time(entry);
        let relative = entry.relative_time();
        let info_text = match (entry.status, entry.row_count) {
            (QueryStatus::Success, Some(rows)) => format!(
                "{}, {} row{} · {}",
                time_str,
                rows,
                if rows == 1 { "" } else { "s" },
                relative
            ),
            (QueryStatus::Error, _) => format!("{}, error · {}", time_str, relative),
            (QueryStatus::Cancelled, _) => format!("cancelled · {}", relative),
            _ => format!("{} · {}", time_str, relative),
        };
        let info_span = Span::styled(info_text, Style::default().fg(Color::DarkGray));

        let mut lines = Vec::new();

        // Add separator line if requested (FR-9.1)
        if with_separator {
            lines.push(Line::from(Span::styled(
                "─".repeat(width.min(30)),
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines.push(Line::from(vec![sql_span]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            source_icon,
            status_icon,
            info_span,
        ]));

        ListItem::new(lines)
    }
}

impl Widget for Sidebar<'_> {
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

        if self.entries.is_empty() {
            // Show placeholder when no queries
            let placeholder = vec![Line::from(Span::styled(
                "No queries yet",
                Style::default().fg(Color::DarkGray),
            ))];
            let paragraph = ratatui::widgets::Paragraph::new(placeholder).block(block);
            paragraph.render(area, buf);
            return;
        }

        // Calculate inner width for formatting
        let inner_width = area.width.saturating_sub(2) as usize;

        // Build list items with separators between time-grouped queries (FR-9.1)
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                // Check if we need a separator before this entry
                let needs_separator = if i > 0 {
                    let prev = &self.entries[i - 1];
                    let time_gap = prev.timestamp.duration_since(entry.timestamp).as_secs();
                    time_gap > 60
                } else {
                    false
                };
                Self::make_list_item_with_separator(entry, inner_width, needs_separator)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("» ");

        // Use stateful rendering for selection
        let mut state = ListState::default();
        state.select(self.selected);

        StatefulWidget::render(list, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};
    use std::time::Duration;

    #[test]
    fn test_format_time_milliseconds() {
        let entry = QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(23), 1);
        assert_eq!(Sidebar::format_time(&entry), "23ms");
    }

    #[test]
    fn test_format_time_seconds() {
        let entry = QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(1500), 1);
        assert_eq!(Sidebar::format_time(&entry), "1.5s");
    }

    #[test]
    fn test_sql_preview_short() {
        let entry = QueryLogEntry::success(
            "SELECT * FROM users".to_string(),
            Duration::from_millis(10),
            5,
        );
        assert_eq!(entry.sql_preview(30), "SELECT * FROM users");
    }

    #[test]
    fn test_sql_preview_truncated() {
        let entry = QueryLogEntry::success(
            "SELECT id, email, name, created_at FROM users WHERE status = 'active'".to_string(),
            Duration::from_millis(10),
            5,
        );
        assert_eq!(entry.sql_preview(30), "SELECT id, email, name, create");
    }

    #[test]
    fn test_cancelled_entry_renders_status() {
        let entry = QueryLogEntry::cancelled_with_source(
            "SELECT 1".to_string(),
            QuerySource::Manual,
        );
        let sidebar = Sidebar::new(&[entry], None, false);
        let area = Rect::new(0, 0, 50, 6);
        let mut buf = Buffer::empty(area);

        sidebar.render(area, &mut buf);

        let rendered: String = buf.content.iter().map(|cell| cell.symbol()).collect();
        assert!(rendered.contains("cancelled"));
    }
}
