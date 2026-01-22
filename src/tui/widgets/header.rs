//! Header widget for the TUI.
//!
//! Displays the application name, version, and database connection info.

use super::spinner::Spinner;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Header bar widget.
pub struct Header<'a> {
    connection_info: Option<&'a str>,
    spinner: Option<&'a Spinner>,
    is_connected: bool,
}

impl<'a> Header<'a> {
    /// Creates a new header widget.
    pub fn new(
        connection_info: Option<&'a str>,
        spinner: Option<&'a Spinner>,
        is_connected: bool,
    ) -> Self {
        Self {
            connection_info,
            spinner,
            is_connected,
        }
    }
}

impl Widget for Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        let style = Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        for x in area.left()..area.right() {
            buf[(x, area.y)].set_style(style);
        }

        // Left side: app name and version
        let left_text = format!(" Glance v{}", env!("CARGO_PKG_VERSION"));
        let left_span = Span::styled(left_text, style);
        buf.set_span(area.x, area.y, &left_span, area.width);

        // Center: spinner if active
        if let Some(spinner) = self.spinner {
            let spinner_text = spinner.display();
            let spinner_style = Style::default()
                .bg(Color::Blue)
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
            let spinner_width = spinner_text.len() as u16;
            let spinner_x = area.x + (area.width.saturating_sub(spinner_width)) / 2;
            buf.set_string(spinner_x, area.y, &spinner_text, spinner_style);
        }

        // Right side: connection status indicator and info
        if let Some(info) = self.connection_info {
            // Connection status dot
            let status_dot = if self.is_connected { "●" } else { "○" };
            let status_color = if self.is_connected {
                Color::Green
            } else {
                Color::Gray
            };
            let status_style = Style::default().bg(Color::Blue).fg(status_color);

            let right_text = format!(" {} [db: {}] ", status_dot, info);
            let right_width = right_text.len() as u16;
            if right_width < area.width {
                let right_x = area.right().saturating_sub(right_width);
                // Render the status dot with its color
                buf.set_string(right_x, area.y, " ", style);
                buf.set_string(right_x + 1, area.y, status_dot, status_style);
                // Render the rest with normal style
                let db_text = format!(" [db: {}] ", info);
                buf.set_string(right_x + 2, area.y, &db_text, style);
            }
        }
    }
}

/// Creates a header line for use in other contexts.
#[allow(dead_code)]
pub fn header_line(connection_info: Option<&str>) -> Line<'_> {
    let style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![Span::styled(format!("Glance v{}", env!("CARGO_PKG_VERSION")), style)];

    if let Some(info) = connection_info {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[db: {}]", info),
            Style::default().fg(Color::Gray),
        ));
    }

    Line::from(spans)
}
