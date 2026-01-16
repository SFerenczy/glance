//! Confirmation dialog widget for mutation/destructive queries.
//!
//! Displays a modal dialog asking the user to confirm execution of
//! queries that modify or delete data.

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::safety::{ClassificationResult, SafetyLevel};

/// Renders a confirmation dialog for a pending query.
///
/// The dialog displays the SQL query and asks the user to confirm execution.
/// The appearance varies based on the safety level:
/// - Mutating: Yellow warning
/// - Destructive: Red warning with additional caution text
pub fn render_confirmation_dialog(
    frame: &mut Frame,
    sql: &str,
    classification: &ClassificationResult,
) {
    let area = frame.area();

    // Calculate dialog size (60% width, up to 15 lines height)
    let dialog_width = (area.width as f32 * 0.6).min(80.0) as u16;
    let dialog_height = calculate_dialog_height(sql, dialog_width).min(15);

    // Center the dialog
    let dialog_area = center_rect(dialog_width, dialog_height, area);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Determine colors based on safety level
    let (border_color, title, icon) = match classification.level {
        SafetyLevel::Safe => (Color::Green, "Confirm Query", "✓"),
        SafetyLevel::Mutating => (Color::Yellow, "Confirm Modification", "⚠"),
        SafetyLevel::Destructive => (Color::Red, "Warning: Destructive Query", "🛑"),
    };

    // Build the dialog content
    let mut lines = Vec::new();

    // Header line with icon
    let header_text = match classification.level {
        SafetyLevel::Safe => "This query will be executed:",
        SafetyLevel::Mutating => "This query will modify data:",
        SafetyLevel::Destructive => "WARNING: This query may cause data loss:",
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{} ", icon), Style::default().fg(border_color)),
        Span::styled(
            header_text,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // SQL query (truncated if too long)
    let sql_lines = wrap_sql(sql, dialog_width.saturating_sub(4) as usize);
    for sql_line in sql_lines.iter().take(5) {
        lines.push(Line::from(Span::styled(
            format!("  {}", sql_line),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        )));
    }
    if sql_lines.len() > 5 {
        lines.push(Line::from(Span::styled(
            "  ...",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));

    // Warning message if present
    if let Some(warning) = &classification.warning {
        lines.push(Line::from(Span::styled(
            warning.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )));
        lines.push(Line::from(""));
    }

    // Prompt
    lines.push(Line::from(vec![
        Span::raw("Execute? "),
        Span::styled(
            "[y/Enter]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Yes  "),
        Span::styled(
            "[n/Esc]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" No"),
    ]));

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, dialog_area);
}

/// Calculates the required height for the dialog based on SQL length.
fn calculate_dialog_height(sql: &str, width: u16) -> u16 {
    let content_width = width.saturating_sub(4) as usize;
    let sql_lines = wrap_sql(sql, content_width).len().min(6);

    // Header (2) + SQL lines + spacing (2) + prompt (1) + borders (2)
    (2 + sql_lines + 2 + 1 + 2) as u16
}

/// Wraps SQL text to fit within the given width.
fn wrap_sql(sql: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let sql = sql.trim();

    for line in sql.lines() {
        if line.len() <= max_width {
            lines.push(line.to_string());
        } else {
            // Simple word wrapping
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else if current_line.len() + 1 + word.len() <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    current_line = word.to_string();
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }
    }

    if lines.is_empty() {
        lines.push(sql.to_string());
    }

    lines
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
    use crate::safety::StatementType;

    #[test]
    fn test_wrap_sql_short() {
        let sql = "SELECT * FROM users";
        let lines = wrap_sql(sql, 50);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], sql);
    }

    #[test]
    fn test_wrap_sql_long() {
        let sql = "SELECT id, name, email, created_at FROM users WHERE active = true";
        let lines = wrap_sql(sql, 30);
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_wrap_sql_multiline() {
        let sql = "SELECT *\nFROM users\nWHERE active = true";
        let lines = wrap_sql(sql, 50);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_calculate_dialog_height() {
        let sql = "SELECT * FROM users";
        let height = calculate_dialog_height(sql, 60);
        assert!(height >= 7); // Minimum height for short query
        assert!(height <= 15); // Maximum height
    }

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

    #[test]
    fn test_classification_colors() {
        // Just verify the function doesn't panic with different levels
        let safe = ClassificationResult::new(SafetyLevel::Safe, StatementType::Select);
        let mutating = ClassificationResult::new(SafetyLevel::Mutating, StatementType::Insert);
        let destructive = ClassificationResult::with_warning(
            SafetyLevel::Destructive,
            StatementType::Delete,
            "This action cannot be undone.",
        );

        // These would need a terminal to actually render, but we can verify
        // the classification result properties
        assert!(!safe.requires_confirmation());
        assert!(mutating.requires_confirmation());
        assert!(destructive.requires_confirmation());
        assert!(destructive.requires_warning());
    }
}
