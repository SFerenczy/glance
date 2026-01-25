//! History selection widget for the TUI.
//!
//! Provides a floating overlay showing input history for selection.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Widget},
};

/// History selection popup widget.
pub struct HistorySelectionPopup<'a> {
    entries: &'a [String],
    selected: usize,
}

impl<'a> HistorySelectionPopup<'a> {
    /// Creates a new history selection popup widget.
    pub fn new(entries: &'a [String], selected: usize) -> Self {
        Self { entries, selected }
    }

    /// Calculates the area for the history popup.
    pub fn popup_area(input_area: Rect) -> Rect {
        // Position above the input bar
        let width = input_area.width.min(80);
        let height = 12.min(input_area.y.saturating_sub(2)); // Leave room for other UI

        let x = input_area.x + 1; // Align with input content
        let y = input_area.y.saturating_sub(height);

        Rect::new(x, y, width, height)
    }
}

impl Widget for HistorySelectionPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area first
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Input History (↑↓ to select, Enter to load) ");

        let inner = block.inner(area);
        block.render(area, buf);

        if self.entries.is_empty() {
            // Show "No history" message
            let msg = "No input history available";
            let msg_style = Style::default().fg(Color::DarkGray);
            let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
            let y = inner.y + inner.height / 2;
            buf.set_string(x, y, msg, msg_style);
            return;
        }

        // Render each history entry
        let mut y = inner.y;
        let max_items = inner.height as usize;

        // Calculate scroll offset to keep selected item visible
        let scroll_offset = if self.selected >= max_items {
            self.selected - max_items + 1
        } else {
            0
        };

        for (idx, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_items)
        {
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = idx == self.selected;

            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Clear line with background if selected
            if is_selected {
                for x in inner.x..inner.x + inner.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(style);
                    }
                }
            }

            // Truncate long entries
            let max_width = inner.width.saturating_sub(2) as usize;
            let display_entry = if entry.len() > max_width {
                format!("{}...", &entry[..max_width.saturating_sub(3)])
            } else {
                entry.clone()
            };

            // Render the entry
            let x = inner.x + 1;
            buf.set_string(x, y, &display_entry, style);

            y += 1;
        }
    }
}
