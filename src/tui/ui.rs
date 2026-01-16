//! UI rendering for the TUI.
//!
//! Defines the layout and renders all UI components.

use super::app::{App, Focus};
use super::widgets::{chat, header, input, sidebar};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

/// Renders the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: header, content, input
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(3),    // Content (chat + sidebar)
            Constraint::Length(3), // Input
        ])
        .split(area);

    let header_area = main_layout[0];
    let content_area = main_layout[1];
    let input_area = main_layout[2];

    // Content layout: chat (70%) and sidebar (30%)
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Chat panel
            Constraint::Percentage(30), // Sidebar
        ])
        .split(content_area);

    let chat_area = content_layout[0];
    let sidebar_area = content_layout[1];

    // Render components
    render_header(frame, header_area, app);
    render_chat(frame, chat_area, app);
    render_sidebar(frame, sidebar_area, app);
    render_input(frame, input_area, app);
}

/// Renders the header bar.
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let widget = header::Header::new(app.connection_info.as_deref());
    frame.render_widget(widget, area);
}

/// Renders the chat panel.
fn render_chat(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Chat;
    let widget = chat::ChatPanel::new(focused);
    frame.render_widget(widget, area);
}

/// Renders the sidebar.
fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Sidebar;
    let widget = sidebar::Sidebar::new(focused);
    frame.render_widget(widget, area);
}

/// Renders the input bar.
fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Input;
    let widget = input::InputBar::new(&app.input.text, app.input.cursor, focused);
    frame.render_widget(widget, area);

    // Position cursor in input field when focused
    if focused {
        // Account for border (1) and prompt "> " (2)
        let cursor_x = area.x + 1 + 2 + app.input.cursor as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
