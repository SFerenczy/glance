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
    queue_depth: usize,
    show_secret_warning: bool,
}

impl<'a> Header<'a> {
    /// Creates a new header widget.
    pub fn new(
        connection_info: Option<&'a str>,
        spinner: Option<&'a Spinner>,
        is_connected: bool,
        queue_depth: usize,
        show_secret_warning: bool,
    ) -> Self {
        Self {
            connection_info,
            spinner,
            is_connected,
            queue_depth,
            show_secret_warning,
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
        let left_text_len = left_text.len() as u16;
        let left_span = Span::styled(left_text, style);
        buf.set_span(area.x, area.y, &left_span, area.width);

        // Warning badge if secrets are stored in plaintext
        if self.show_secret_warning {
            let warning_text = " ⚠️  Secrets in plaintext ";
            let warning_x = area.x + left_text_len;
            let warning_style = Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);
            buf.set_string(warning_x, area.y, warning_text, warning_style);
        }

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

        // Right side: queue depth (if > 0) + connection status indicator and info
        let mut right_spans = Vec::new();

        // Add queue indicator if there are queued requests
        if self.queue_depth > 0 {
            right_spans.push(Span::styled(
                format!("Queue: {} ", self.queue_depth),
                Style::default().bg(Color::Blue).fg(Color::Yellow),
            ));
        }

        // Add connection info
        if let Some(info) = self.connection_info {
            // Connection status dot and text
            let (status_dot, status_text, status_color) = if self.is_connected {
                ("●", "connected", Color::Green)
            } else {
                ("○", "disconnected", Color::Red)
            };
            let status_style = Style::default().bg(Color::Blue).fg(status_color);

            right_spans.push(Span::styled(
                " ",
                Style::default().bg(Color::Blue).fg(Color::White),
            ));
            right_spans.push(Span::styled(status_dot, status_style));
            right_spans.push(Span::styled(
                " ",
                Style::default().bg(Color::Blue).fg(Color::White),
            ));
            right_spans.push(Span::styled(status_text, status_style));
            right_spans.push(Span::styled(
                format!(" [db: {}] ", info),
                Style::default().bg(Color::Blue).fg(Color::White),
            ));
        } else {
            // No connection configured
            right_spans.push(Span::styled(
                " Not connected - use /conn add ",
                Style::default().bg(Color::Blue).fg(Color::Yellow),
            ));
        }

        // Render right side
        if !right_spans.is_empty() {
            let right_line = Line::from(right_spans);
            let right_width = right_line.width() as u16;
            if right_width < area.width {
                let right_x = area.right().saturating_sub(right_width);
                buf.set_line(right_x, area.y, &right_line, right_width);
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

    let mut spans = vec![Span::styled(
        format!("Glance v{}", env!("CARGO_PKG_VERSION")),
        style,
    )];

    if let Some(info) = connection_info {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[db: {}]", info),
            Style::default().fg(Color::Gray),
        ));
    }

    Line::from(spans)
}
