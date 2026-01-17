//! UI rendering for the TUI.
//!
//! Defines the layout and renders all UI components.

use super::app::{App, Focus};
use super::widgets::{chat, command_palette, confirm, header, input, query_detail, sidebar, toast};
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

    // Render modal overlay if query detail is shown
    if app.show_query_detail {
        if let Some(entry) = app.selected_query_entry() {
            let modal = query_detail::QueryDetailModal::new(entry);
            frame.render_widget(modal, area);
        }
    }

    // Render confirmation dialog if there's a pending query
    if let Some(pending) = &app.pending_query {
        confirm::render_confirmation_dialog(frame, &pending.sql, &pending.classification);
    }

    // Render command palette if visible
    if app.command_palette.visible {
        let palette_area = command_palette::CommandPalette::popup_area(input_area);
        let palette = command_palette::CommandPalette::new(&app.command_palette);
        frame.render_widget(palette, palette_area);
    }

    // Render toast notification if present
    if let Some((message, _)) = &app.toast {
        let toast_area = toast::Toast::area(area);
        let toast_widget = toast::Toast::new(message);
        frame.render_widget(toast_widget, toast_area);
    }
}

/// Renders the header bar.
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let widget = header::Header::new(app.connection_info.as_deref());
    frame.render_widget(widget, area);
}

/// Renders the chat panel.
fn render_chat(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Chat;
    let widget = chat::ChatPanel::new(&app.messages, app.chat_scroll, focused);
    frame.render_widget(widget, area);
}

/// Renders the sidebar.
fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Sidebar;
    let widget = sidebar::Sidebar::new(&app.query_log, app.selected_query, focused);
    frame.render_widget(widget, area);
}

/// Renders the input bar.
fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Input;
    let widget = input::InputBar::new(&app.input.text, app.input.cursor, focused, app.input_mode);
    frame.render_widget(widget, area);

    // Position cursor in input field when focused
    if focused {
        // Account for border (1) and prompt "> " (2)
        let cursor_x = area.x + 1 + 2 + app.input.cursor as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
