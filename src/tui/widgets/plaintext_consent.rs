//! Plaintext storage consent dialog widget.
//!
//! Displays a modal dialog asking the user to consent to plaintext storage
//! when the OS keyring is unavailable.

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Renders a consent dialog for plaintext secret storage.
///
/// Displayed when the user attempts to save a secret (password or API key)
/// but the OS keyring is unavailable. The user must consent before secrets
/// can be stored in plaintext.
pub fn render_plaintext_consent_dialog(frame: &mut Frame) {
    let area = frame.area();

    // Fixed dialog dimensions
    let dialog_width = 60u16.min(area.width.saturating_sub(4));
    let dialog_height = 11u16;

    // Center the dialog
    let dialog_area = center_rect(dialog_width, dialog_height, area);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Build the dialog content
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No secure keyring found. Secrets will be stored in",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "plaintext on disk if you continue.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "To avoid this, install a keyring service",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "(e.g., gnome-keyring, kwallet, or secret-service).",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y/Enter]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Continue  "),
            Span::styled(
                "[n/Esc]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ]),
    ];

    let block = Block::default()
        .title(" Plaintext Storage ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, dialog_area);
}

/// Centers a rectangle of the given size within the parent area.
fn center_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = center_rect(40, 10, area);

        // Should be roughly centered
        assert!(centered.x >= 25 && centered.x <= 35);
        assert!(centered.y >= 15 && centered.y <= 25);
        assert_eq!(centered.width, 40);
        assert_eq!(centered.height, 10);
    }
}
