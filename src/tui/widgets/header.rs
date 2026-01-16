//! Header widget for the TUI.
//!
//! Displays the application name, version, and database connection info.

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
}

impl<'a> Header<'a> {
    /// Creates a new header widget.
    pub fn new(connection_info: Option<&'a str>) -> Self {
        Self { connection_info }
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
        let left_text = " Glance v0.1.0";
        let left_span = Span::styled(left_text, style);
        buf.set_span(area.x, area.y, &left_span, area.width);

        // Right side: connection info
        if let Some(info) = self.connection_info {
            let right_text = format!("[db: {}] ", info);
            let right_width = right_text.len() as u16;
            if right_width < area.width {
                let right_x = area.right().saturating_sub(right_width);
                let right_span = Span::styled(right_text, style);
                buf.set_span(right_x, area.y, &right_span, right_width);
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

    let mut spans = vec![Span::styled("Glance v0.1.0", style)];

    if let Some(info) = connection_info {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[db: {}]", info),
            Style::default().fg(Color::Gray),
        ));
    }

    Line::from(spans)
}
