//! Help overlay widget for the TUI.
//!
//! Displays keyboard shortcuts and commands.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Help overlay widget showing keyboard shortcuts.
pub struct HelpOverlay;

impl HelpOverlay {
    /// Creates a new help overlay widget.
    pub fn new() -> Self {
        Self
    }

    /// Calculates the centered area for the help overlay.
    pub fn area(parent: Rect) -> Rect {
        let width = 50.min(parent.width.saturating_sub(4));
        let height = 22.min(parent.height.saturating_sub(4));
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }

    /// Returns the help content as lines.
    fn content() -> Vec<Line<'static>> {
        let key_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Color::White);
        let section_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        vec![
            Line::from(Span::styled("Normal Mode", section_style)),
            Self::shortcut_line("i", "Enter Insert mode", key_style, desc_style),
            Self::shortcut_line("Esc", "Exit to Normal mode", key_style, desc_style),
            Self::shortcut_line("y", "Copy last SQL to clipboard", key_style, desc_style),
            Self::shortcut_line("e", "Edit last SQL", key_style, desc_style),
            Self::shortcut_line("r", "Re-run last SQL", key_style, desc_style),
            Self::shortcut_line("j/k", "Scroll chat down/up", key_style, desc_style),
            Self::shortcut_line("g/G", "Go to top/bottom", key_style, desc_style),
            Self::shortcut_line("Ctrl+d/u", "Half page down/up", key_style, desc_style),
            Self::shortcut_line("?", "Toggle this help", key_style, desc_style),
            Line::from(""),
            Line::from(Span::styled("Insert Mode", section_style)),
            Self::shortcut_line("/", "Open command palette", key_style, desc_style),
            Self::shortcut_line("↑/↓", "Navigate input history", key_style, desc_style),
            Self::shortcut_line("Enter", "Submit input", key_style, desc_style),
            Self::shortcut_line("Ctrl+U", "Clear input", key_style, desc_style),
            Line::from(""),
            Line::from(Span::styled("General", section_style)),
            Self::shortcut_line("Tab", "Cycle focus", key_style, desc_style),
            Self::shortcut_line("Ctrl+C/Q", "Quit", key_style, desc_style),
        ]
    }

    /// Creates a line with a keyboard shortcut and description.
    fn shortcut_line(
        key: &'static str,
        desc: &'static str,
        key_style: Style,
        desc_style: Style,
    ) -> Line<'static> {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:12}", key), key_style),
            Span::styled(desc, desc_style),
        ])
    }
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area first
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Help (? to close) ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );

        let content = Self::content();
        let paragraph = Paragraph::new(content).block(block);

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_area() {
        let parent = Rect::new(0, 0, 100, 50);
        let area = HelpOverlay::area(parent);
        assert!(area.width <= 50);
        assert!(area.height <= 22);
        assert!(area.x > 0);
        assert!(area.y > 0);
    }

    #[test]
    fn test_help_content() {
        let content = HelpOverlay::content();
        assert!(!content.is_empty());
    }
}
