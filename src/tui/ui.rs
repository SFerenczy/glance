//! UI rendering for the TUI.
//!
//! Defines the layout and renders all UI components.

use super::app::{App, Focus};
use super::widgets::{
    chat, command_palette, confirm, header, help, input, query_detail, sidebar, toast,
};
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

    // Content layout: dynamic sizing based on focus
    // When sidebar is focused, give it more space (40%), otherwise 30%
    let (chat_pct, sidebar_pct) = if app.focus == Focus::Sidebar {
        (60, 40)
    } else {
        (70, 30)
    };
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(chat_pct),
            Constraint::Percentage(sidebar_pct),
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

    // Render help overlay if visible
    if app.show_help {
        let help_area = help::HelpOverlay::area(area);
        let help_widget = help::HelpOverlay::new();
        frame.render_widget(help_widget, help_area);
    }
}

/// Renders the header bar.
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let widget = header::Header::new(
        app.connection_info.as_deref(),
        app.spinner.as_ref(),
        app.is_connected,
    );
    frame.render_widget(widget, area);
}

/// Renders the chat panel.
fn render_chat(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Chat;
    let widget = chat::ChatPanel::new(
        &app.messages,
        app.chat_scroll,
        focused,
        app.has_new_messages,
    );
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
    let widget = input::InputBar::new(
        &app.input.text,
        app.input.cursor,
        focused,
        app.input_mode,
        app.vim_mode_enabled,
    );
    frame.render_widget(widget, area);

    // Position cursor in input field when focused
    if focused {
        // Calculate scroll offset to match the widget's rendering
        // Border left (1) + prompt "> " (2) + border right (1) + cursor space (1) = 5
        let available_width = area.width.saturating_sub(5) as usize;
        let scroll_offset =
            input::calculate_scroll_offset(app.input.cursor, app.input.text.len(), available_width);

        // Account for border (1) and prompt "> " (2), minus scroll offset
        let cursor_x = area.x + 1 + 2 + (app.input.cursor - scroll_offset) as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
