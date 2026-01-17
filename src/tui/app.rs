//! Application state for the TUI.
//!
//! Contains the main App struct and related types for managing UI state.

use super::history::InputHistory;
use super::widgets::command_palette::CommandPaletteState;
use super::widgets::spinner::Spinner;
use crate::config::ConnectionConfig;
use crate::db::QueryResult;
use std::time::{Duration, Instant};

/// Status of an executed query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    /// Query executed successfully.
    Success,
    /// Query failed with an error.
    Error,
}

/// An entry in the query log.
#[derive(Debug, Clone)]
pub struct QueryLogEntry {
    /// The SQL query that was executed.
    pub sql: String,
    /// Whether the query succeeded or failed.
    pub status: QueryStatus,
    /// How long the query took to execute.
    pub execution_time: Duration,
    /// Number of rows returned (for successful queries).
    pub row_count: Option<usize>,
    /// Error message (for failed queries).
    pub error: Option<String>,
    /// When the query was executed.
    pub timestamp: Instant,
}

impl QueryLogEntry {
    /// Creates a new successful query log entry.
    pub fn success(sql: String, execution_time: Duration, row_count: usize) -> Self {
        Self {
            sql,
            status: QueryStatus::Success,
            execution_time,
            row_count: Some(row_count),
            error: None,
            timestamp: Instant::now(),
        }
    }

    /// Creates a new failed query log entry.
    pub fn error(sql: String, execution_time: Duration, error: String) -> Self {
        Self {
            sql,
            status: QueryStatus::Error,
            execution_time,
            row_count: None,
            error: Some(error),
            timestamp: Instant::now(),
        }
    }

    /// Returns a human-readable relative timestamp.
    pub fn relative_time(&self) -> String {
        let elapsed = self.timestamp.elapsed();
        let secs = elapsed.as_secs();

        if secs < 60 {
            "just now".to_string()
        } else if secs < 3600 {
            let mins = secs / 60;
            format!("{}m ago", mins)
        } else if secs < 86400 {
            let hours = secs / 3600;
            format!("{}h ago", hours)
        } else {
            let days = secs / 86400;
            format!("{}d ago", days)
        }
    }

    /// Returns a truncated preview of the SQL (first 30 chars).
    pub fn sql_preview(&self, max_len: usize) -> &str {
        let sql = self.sql.trim();
        if sql.len() <= max_len {
            sql
        } else {
            // Find a safe truncation point (don't split UTF-8)
            let mut end = max_len;
            while end > 0 && !sql.is_char_boundary(end) {
                end -= 1;
            }
            &sql[..end]
        }
    }
}

/// Which panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Input,
    Chat,
    Sidebar,
}

/// Input mode for vim-style editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Normal mode: navigation and commands.
    #[default]
    Normal,
    /// Insert mode: text input.
    Insert,
}

impl InputMode {
    /// Returns the display string for the mode indicator.
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Normal => "-- NORMAL --",
            Self::Insert => "-- INSERT --",
        }
    }
}

impl Focus {
    /// Cycles to the next focus panel.
    pub fn next(self) -> Self {
        match self {
            Self::Input => Self::Chat,
            Self::Chat => Self::Sidebar,
            Self::Sidebar => Self::Input,
        }
    }
}

/// A message in the chat panel.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// A message from the user.
    User(String),
    /// A response from the assistant.
    Assistant(String),
    /// A query result to display as a table.
    Result(QueryResult),
    /// An error message.
    Error(String),
    /// A system message (e.g., schema display, help text).
    System(String),
}

#[allow(dead_code)]
impl ChatMessage {
    /// Returns the message type as a string for display purposes.
    pub fn type_label(&self) -> &'static str {
        match self {
            Self::User(_) => "You",
            Self::Assistant(_) => "Glance",
            Self::Result(_) => "Result",
            Self::Error(_) => "Error",
            Self::System(_) => "System",
        }
    }
}

/// Input state for text editing.
#[derive(Debug, Default)]
pub struct InputState {
    /// Current input text.
    pub text: String,
    /// Cursor position (character index).
    pub cursor: usize,
}

impl InputState {
    /// Creates a new empty input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a character at the cursor position.
    pub fn insert(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Deletes the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
        }
    }

    /// Deletes the character at the cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Moves the cursor left.
    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Moves the cursor right.
    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    /// Moves the cursor to the start of the input.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Moves the cursor to the end of the input.
    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Clears the input and returns the previous text.
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    /// Returns true if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clears the input text and resets cursor.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }
}

/// Main application state.
pub struct App {
    /// Whether the application is still running.
    pub running: bool,
    /// Current focus panel.
    pub focus: Focus,
    /// Current input mode (Normal/Insert).
    pub input_mode: InputMode,
    /// Input field state.
    pub input: InputState,
    /// Input history for arrow key navigation.
    pub input_history: InputHistory,
    /// Command palette state.
    pub command_palette: CommandPaletteState,
    /// Chat messages.
    pub messages: Vec<ChatMessage>,
    /// Chat scroll offset (lines from bottom).
    pub chat_scroll: usize,
    /// Whether there are new messages below the current scroll position.
    pub has_new_messages: bool,
    /// Query log entries.
    pub query_log: Vec<QueryLogEntry>,
    /// Currently selected query in sidebar (index into query_log).
    pub selected_query: Option<usize>,
    /// Whether the query detail modal is visible.
    pub show_query_detail: bool,
    /// Database connection info for display.
    pub connection_info: Option<String>,
    /// Pending query awaiting confirmation.
    pub pending_query: Option<PendingQuery>,
    /// Whether the application is currently processing (waiting for LLM/DB).
    pub is_processing: bool,
    /// Active spinner for visual feedback during async operations.
    pub spinner: Option<Spinner>,
    /// Last executed SQL query (for copy/re-run features).
    pub last_executed_sql: Option<String>,
    /// Timestamp of last Esc press (for double-Esc detection).
    #[allow(dead_code)] // Used in Phase 8.1
    pub last_esc_time: Option<Instant>,
    /// Toast notification message and expiry time.
    pub toast: Option<(String, Instant)>,
    /// Flag indicating a re-run of the last SQL was requested.
    pub rerun_requested: bool,
    /// Whether the help overlay is visible.
    pub show_help: bool,
    /// Whether to ring the terminal bell on next render (for long query notification).
    pub ring_bell: bool,
    /// Whether the database connection is active/healthy.
    pub is_connected: bool,
    /// Whether vim-style navigation is enabled (toggled via /vim command).
    pub vim_mode_enabled: bool,
}

/// A query that is pending user confirmation.
#[derive(Debug, Clone)]
pub struct PendingQuery {
    /// The SQL to execute.
    pub sql: String,
    /// The safety classification of the query.
    pub classification: crate::safety::ClassificationResult,
}

impl App {
    /// Creates a new App instance.
    pub fn new(connection: Option<&ConnectionConfig>) -> Self {
        let connection_info = connection.map(|c| c.display_string());

        // Add welcome message
        let messages = vec![ChatMessage::System(
            "Welcome to Glance! Ask questions about your database in natural language.".to_string(),
        )];

        Self {
            running: true,
            focus: Focus::default(),
            input_mode: InputMode::Insert, // Start in Insert mode for immediate typing
            input: InputState::new(),
            input_history: InputHistory::new(),
            command_palette: CommandPaletteState::new(),
            messages,
            chat_scroll: 0,
            has_new_messages: false,
            query_log: Vec::new(),
            selected_query: None,
            show_query_detail: false,
            connection_info,
            pending_query: None,
            is_processing: false,
            spinner: None,
            last_executed_sql: None,
            last_esc_time: None,
            toast: None,
            rerun_requested: false,
            show_help: false,
            ring_bell: false,
            is_connected: true,      // Assume connected initially
            vim_mode_enabled: false, // Vim mode disabled by default
        }
    }

    /// Shows a toast notification that expires after a duration.
    pub fn show_toast(&mut self, message: impl Into<String>) {
        let expiry = Instant::now() + Duration::from_secs(3);
        self.toast = Some((message.into(), expiry));
    }

    /// Starts the LLM thinking spinner.
    #[allow(dead_code)] // Called from TUI event loop
    pub fn start_thinking(&mut self) {
        self.is_processing = true;
        self.spinner = Some(Spinner::thinking());
    }

    /// Starts the query execution spinner.
    #[allow(dead_code)] // Called from TUI event loop
    pub fn start_executing(&mut self) {
        self.is_processing = true;
        self.spinner = Some(Spinner::executing());
    }

    /// Stops any active spinner.
    #[allow(dead_code)] // Called from TUI event loop
    pub fn stop_spinner(&mut self) {
        self.is_processing = false;
        self.spinner = None;
    }

    /// Requests a terminal bell (for long query notification).
    #[allow(dead_code)] // Called from TUI event loop
    pub fn request_bell(&mut self) {
        self.ring_bell = true;
    }

    /// Takes and clears the bell request.
    pub fn take_bell_request(&mut self) -> bool {
        std::mem::take(&mut self.ring_bell)
    }

    /// Clears expired toast notifications.
    pub fn clear_expired_toast(&mut self) {
        if let Some((_, expiry)) = &self.toast {
            if Instant::now() > *expiry {
                self.toast = None;
            }
        }
    }

    /// Toggles vim mode on/off.
    pub fn toggle_vim_mode(&mut self) {
        self.vim_mode_enabled = !self.vim_mode_enabled;
        if self.vim_mode_enabled {
            self.show_toast("Vim mode enabled");
            // Start in Insert mode when enabling vim mode
            self.input_mode = InputMode::Insert;
        } else {
            self.show_toast("Vim mode disabled");
            // Reset to Insert mode when disabling
            self.input_mode = InputMode::Insert;
        }
    }

    /// Returns true if a confirmation dialog should be shown.
    pub fn has_pending_query(&self) -> bool {
        self.pending_query.is_some()
    }

    /// Sets a pending query that needs confirmation.
    pub fn set_pending_query(
        &mut self,
        sql: String,
        classification: crate::safety::ClassificationResult,
    ) {
        self.pending_query = Some(PendingQuery {
            sql,
            classification,
        });
    }

    /// Clears the pending query.
    pub fn clear_pending_query(&mut self) {
        self.pending_query = None;
    }

    /// Takes the pending query, returning it and clearing the state.
    pub fn take_pending_query(&mut self) -> Option<PendingQuery> {
        self.pending_query.take()
    }

    /// Adds a message to the chat.
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        // If user has scrolled up, mark that there are new messages
        if self.chat_scroll > 0 {
            self.has_new_messages = true;
        } else {
            // Auto-scroll to bottom when at bottom
            self.chat_scroll = 0;
        }
    }

    /// Clears all chat messages.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.chat_scroll = 0;
    }

    /// Adds a query to the log.
    pub fn add_query_log(&mut self, entry: QueryLogEntry) {
        // Insert at the beginning (most recent first)
        self.query_log.insert(0, entry);
        // Update selection to stay on the same item or select the new one
        if self.selected_query.is_some() {
            self.selected_query = self.selected_query.map(|i| i + 1);
        }
    }

    /// Returns the currently selected query entry, if any.
    pub fn selected_query_entry(&self) -> Option<&QueryLogEntry> {
        self.selected_query.and_then(|i| self.query_log.get(i))
    }

    /// Moves selection up in the query log.
    pub fn select_previous_query(&mut self) {
        if self.query_log.is_empty() {
            return;
        }
        self.selected_query = Some(match self.selected_query {
            None => 0,
            Some(i) => i.saturating_sub(1),
        });
    }

    /// Moves selection down in the query log.
    pub fn select_next_query(&mut self) {
        if self.query_log.is_empty() {
            return;
        }
        let max_index = self.query_log.len().saturating_sub(1);
        self.selected_query = Some(match self.selected_query {
            None => 0,
            Some(i) => i.saturating_add(1).min(max_index),
        });
    }

    /// Opens the query detail modal for the selected query.
    pub fn open_query_detail(&mut self) {
        if self.selected_query.is_some() {
            self.show_query_detail = true;
        }
    }

    /// Closes the query detail modal.
    pub fn close_query_detail(&mut self) {
        self.show_query_detail = false;
    }

    /// Returns the total number of lines needed to render all messages.
    /// This is used for scroll calculations.
    pub fn total_chat_lines(&self) -> usize {
        self.messages.iter().map(Self::message_line_count).sum()
    }

    /// Estimates the number of lines a message will take to render.
    fn message_line_count(message: &ChatMessage) -> usize {
        match message {
            ChatMessage::User(text)
            | ChatMessage::Assistant(text)
            | ChatMessage::System(text)
            | ChatMessage::Error(text) => {
                // Label line + content lines (rough estimate: 1 line per 80 chars)
                1 + text.len().div_ceil(80).max(1)
            }
            ChatMessage::Result(result) => {
                // Header + column headers + separator + rows + footer
                3 + result.rows.len() + 2
            }
        }
    }

    /// Handles an event and updates application state.
    pub fn handle_event(&mut self, event: super::Event) {
        use super::Event;
        use crossterm::event::KeyCode;

        match event {
            Event::Key(key) => {
                match key.code {
                    // Exit commands
                    KeyCode::Char('c')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        self.running = false;
                    }
                    KeyCode::Char('q')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        self.running = false;
                    }

                    // Focus switching
                    KeyCode::Tab => {
                        self.focus = self.focus.next();
                    }

                    // Input handling (when input is focused)
                    _ if self.focus == Focus::Input => {
                        self.handle_input_key(key);
                    }

                    // Chat scrolling (when chat is focused)
                    KeyCode::Up if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_add(1);
                    }
                    KeyCode::Down if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_sub(1);
                        if self.chat_scroll == 0 {
                            self.has_new_messages = false;
                        }
                    }
                    KeyCode::PageUp if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_add(10);
                    }
                    KeyCode::PageDown if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_sub(10);
                        if self.chat_scroll == 0 {
                            self.has_new_messages = false;
                        }
                    }
                    KeyCode::Home if self.focus == Focus::Chat => {
                        self.chat_scroll = usize::MAX; // Will be clamped during render
                    }
                    KeyCode::End if self.focus == Focus::Chat => {
                        self.chat_scroll = 0;
                        self.has_new_messages = false;
                    }

                    // Modal handling (Esc closes modal)
                    KeyCode::Esc if self.show_query_detail => {
                        self.close_query_detail();
                    }

                    // Sidebar navigation (when sidebar is focused)
                    KeyCode::Up if self.focus == Focus::Sidebar => {
                        self.select_previous_query();
                    }
                    KeyCode::Down if self.focus == Focus::Sidebar => {
                        self.select_next_query();
                    }
                    KeyCode::Enter if self.focus == Focus::Sidebar => {
                        self.open_query_detail();
                    }

                    _ => {}
                }
            }
            Event::Resize(width, height) => {
                // Terminal resize: clamp scroll position to valid range
                // The actual re-render is handled automatically by ratatui
                self.handle_resize(width, height);
            }
            Event::Tick => {
                // Periodic tick for animations/updates (not used yet)
            }
        }
    }

    /// Handles terminal resize by clamping scroll positions.
    fn handle_resize(&mut self, _width: u16, _height: u16) {
        // Clamp chat scroll to valid range based on content
        let max_scroll = self.total_chat_lines().saturating_sub(1);
        self.chat_scroll = self.chat_scroll.min(max_scroll);

        // Clamp sidebar selection to valid range
        if let Some(selected) = self.selected_query {
            if selected >= self.query_log.len() {
                self.selected_query = if self.query_log.is_empty() {
                    None
                } else {
                    Some(self.query_log.len() - 1)
                };
            }
        }
    }

    /// Handles key events when input is focused.
    fn handle_input_key(&mut self, key: crossterm::event::KeyEvent) {
        // When vim mode is disabled, always use standard input handling
        if !self.vim_mode_enabled {
            self.handle_standard_input_key(key);
            return;
        }

        // Vim mode enabled: use Normal/Insert mode switching
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode_key(key),
            InputMode::Insert => self.handle_insert_mode_key(key),
        }
    }

    /// Handles key events in standard mode (vim mode disabled).
    fn handle_standard_input_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        // Handle command palette if visible
        if self.command_palette.visible {
            match key.code {
                KeyCode::Esc => {
                    self.command_palette.close();
                }
                KeyCode::Up => {
                    self.command_palette.select_previous();
                }
                KeyCode::Down => {
                    self.command_palette.select_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    if let Some(cmd) = self.command_palette.selected_command() {
                        self.input.text = format!("/{} ", cmd.name);
                        self.input.cursor = self.input.text.len();
                    }
                    self.command_palette.close();
                }
                KeyCode::Backspace => {
                    if self.input.text.len() > 1 {
                        self.input.backspace();
                        let filter = self.input.text.strip_prefix('/').unwrap_or("");
                        self.command_palette.set_filter(filter);
                    } else {
                        self.input.backspace();
                        self.command_palette.close();
                    }
                }
                KeyCode::Char(c) => {
                    self.input.insert(c);
                    let filter = self.input.text.strip_prefix('/').unwrap_or("");
                    self.command_palette.set_filter(filter);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            // Esc clears input in standard mode
            KeyCode::Esc => {
                self.input.clear();
            }
            // Clear input with Ctrl+U
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.input.clear();
            }
            // History navigation
            KeyCode::Up => {
                if let Some(entry) = self.input_history.previous(&self.input.text) {
                    self.input.text = entry.to_string();
                    self.input.cursor = self.input.text.len();
                }
            }
            KeyCode::Down => {
                if let Some(entry) = self.input_history.next() {
                    self.input.text = entry.to_string();
                    self.input.cursor = self.input.text.len();
                }
            }
            // Text input
            KeyCode::Char(c) => {
                if c == '/' && self.input.is_empty() {
                    self.input.insert(c);
                    self.command_palette.open();
                } else {
                    self.input.insert(c);
                }
            }
            KeyCode::Backspace => {
                self.input.backspace();
            }
            KeyCode::Delete => {
                self.input.delete();
            }
            KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Right => {
                self.input.move_right();
            }
            KeyCode::Home => {
                self.input.move_home();
            }
            KeyCode::End => {
                self.input.move_end();
            }
            KeyCode::Enter => {
                // Enter is handled by the main event loop for submission
            }
            _ => {}
        }
    }

    /// Handles key events in Normal mode.
    fn handle_normal_mode_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            // Enter Insert mode
            KeyCode::Char('i') => {
                self.input_mode = InputMode::Insert;
            }
            // Enter Insert mode at end of line
            KeyCode::Char('a') => {
                self.input_mode = InputMode::Insert;
                self.input.move_end();
            }
            // Enter Insert mode at start of line
            KeyCode::Char('I') => {
                self.input_mode = InputMode::Insert;
                self.input.move_home();
            }
            // Enter Insert mode at end of line (append)
            KeyCode::Char('A') => {
                self.input_mode = InputMode::Insert;
                self.input.move_end();
            }
            // Navigation in Normal mode
            KeyCode::Char('h') | KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.input.move_right();
            }
            KeyCode::Char('0') | KeyCode::Home => {
                self.input.move_home();
            }
            KeyCode::Char('$') | KeyCode::End => {
                self.input.move_end();
            }
            // Delete character under cursor
            KeyCode::Char('x') => {
                self.input.delete();
            }
            // Copy last SQL to clipboard
            KeyCode::Char('y') => {
                self.copy_last_sql();
            }
            // Edit last SQL - load into input
            KeyCode::Char('e') => {
                self.edit_last_sql();
            }
            // Re-run last SQL
            KeyCode::Char('r') => {
                self.request_rerun();
            }
            // Toggle help overlay
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            // Vim-style scrolling
            KeyCode::Char('j') => {
                self.chat_scroll = self.chat_scroll.saturating_sub(1);
                if self.chat_scroll == 0 {
                    self.has_new_messages = false;
                }
            }
            KeyCode::Char('k') => {
                self.chat_scroll = self.chat_scroll.saturating_add(1);
            }
            KeyCode::Char('g') => {
                // Go to top (oldest messages)
                self.chat_scroll = self.total_chat_lines();
            }
            KeyCode::Char('G') => {
                // Go to bottom (newest messages)
                self.chat_scroll = 0;
                self.has_new_messages = false;
            }
            KeyCode::Char('d')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Half page down
                self.chat_scroll = self.chat_scroll.saturating_sub(10);
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Half page up
                self.chat_scroll = self.chat_scroll.saturating_add(10);
            }
            _ => {}
        }
    }

    /// Copies the last executed SQL to the clipboard.
    fn copy_last_sql(&mut self) {
        if let Some(sql) = &self.last_executed_sql {
            match super::clipboard::copy(sql) {
                Ok(()) => {
                    self.show_toast("Copied SQL to clipboard");
                }
                Err(e) => {
                    self.show_toast(format!("Failed to copy: {}", e));
                }
            }
        } else {
            self.show_toast("No SQL to copy");
        }
    }

    /// Loads the last executed SQL into the input field for editing.
    fn edit_last_sql(&mut self) {
        if let Some(sql) = &self.last_executed_sql.clone() {
            self.input.text = sql.clone();
            self.input.cursor = sql.len();
            self.input_mode = InputMode::Insert;
            self.show_toast("Loaded last SQL for editing");
        } else {
            self.show_toast("No SQL to edit");
        }
    }

    /// Returns the last SQL for re-execution, if any.
    /// The caller should handle actually executing the query.
    #[allow(dead_code)] // Will be used by TUI event loop
    pub fn get_rerun_sql(&self) -> Option<String> {
        self.last_executed_sql.clone()
    }

    /// Requests a re-run of the last SQL query.
    fn request_rerun(&mut self) {
        if self.last_executed_sql.is_some() {
            self.rerun_requested = true;
            self.show_toast("Re-running last SQL...");
        } else {
            self.show_toast("No SQL to re-run");
        }
    }

    /// Takes and clears the rerun request, returning the SQL if requested.
    #[allow(dead_code)] // Will be used by TUI event loop
    pub fn take_rerun_request(&mut self) -> Option<String> {
        if self.rerun_requested {
            self.rerun_requested = false;
            self.last_executed_sql.clone()
        } else {
            None
        }
    }

    /// Handles key events in Insert mode.
    fn handle_insert_mode_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        // Handle command palette if visible
        if self.command_palette.visible {
            match key.code {
                KeyCode::Esc => {
                    self.command_palette.close();
                }
                KeyCode::Up => {
                    self.command_palette.select_previous();
                }
                KeyCode::Down => {
                    self.command_palette.select_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    // Accept selection
                    if let Some(cmd) = self.command_palette.selected_command() {
                        self.input.text = format!("/{} ", cmd.name);
                        self.input.cursor = self.input.text.len();
                    }
                    self.command_palette.close();
                }
                KeyCode::Backspace => {
                    // Update filter or close if empty
                    if self.input.text.len() > 1 {
                        self.input.backspace();
                        // Update filter (text after '/')
                        let filter = self.input.text.strip_prefix('/').unwrap_or("");
                        self.command_palette.set_filter(filter);
                    } else {
                        self.input.backspace();
                        self.command_palette.close();
                    }
                }
                KeyCode::Char(c) => {
                    self.input.insert(c);
                    // Update filter (text after '/')
                    let filter = self.input.text.strip_prefix('/').unwrap_or("");
                    self.command_palette.set_filter(filter);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            // Exit to Normal mode
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            // Clear input with Ctrl+U
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.input.clear();
            }
            // History navigation
            KeyCode::Up => {
                if let Some(entry) = self.input_history.previous(&self.input.text) {
                    self.input.text = entry.to_string();
                    self.input.cursor = self.input.text.len();
                }
            }
            KeyCode::Down => {
                if let Some(entry) = self.input_history.next() {
                    self.input.text = entry.to_string();
                    self.input.cursor = self.input.text.len();
                }
            }
            // Text input
            KeyCode::Char(c) => {
                // Check if this triggers command palette
                if c == '/' && self.input.is_empty() {
                    self.input.insert(c);
                    self.command_palette.open();
                } else {
                    self.input.insert(c);
                }
            }
            KeyCode::Backspace => {
                self.input.backspace();
            }
            KeyCode::Delete => {
                self.input.delete();
            }
            KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Right => {
                self.input.move_right();
            }
            KeyCode::Home => {
                self.input.move_home();
            }
            KeyCode::End => {
                self.input.move_end();
            }
            KeyCode::Enter => {
                // Enter is handled by the main event loop for submission
            }
            _ => {}
        }
    }

    /// Submits the current input for processing.
    pub fn submit_input(&mut self) -> Option<String> {
        if self.input.is_empty() {
            None
        } else {
            let text = self.input.take();
            // Add to history and reset position
            self.input_history.push(text.clone());
            self.input_history.reset_position();
            Some(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_insert() {
        let mut input = InputState::new();
        input.insert('h');
        input.insert('i');
        assert_eq!(input.text, "hi");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_input_backspace() {
        let mut input = InputState::new();
        input.text = "hello".to_string();
        input.cursor = 5;
        input.backspace();
        assert_eq!(input.text, "hell");
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_input_backspace_at_start() {
        let mut input = InputState::new();
        input.text = "hello".to_string();
        input.cursor = 0;
        input.backspace();
        assert_eq!(input.text, "hello");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_input_delete() {
        let mut input = InputState::new();
        input.text = "hello".to_string();
        input.cursor = 0;
        input.delete();
        assert_eq!(input.text, "ello");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_input_cursor_movement() {
        let mut input = InputState::new();
        input.text = "hello".to_string();
        input.cursor = 2;

        input.move_left();
        assert_eq!(input.cursor, 1);

        input.move_right();
        assert_eq!(input.cursor, 2);

        input.move_home();
        assert_eq!(input.cursor, 0);

        input.move_end();
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn test_input_take() {
        let mut input = InputState::new();
        input.text = "hello".to_string();
        input.cursor = 3;

        let text = input.take();
        assert_eq!(text, "hello");
        assert!(input.text.is_empty());
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_focus_cycle() {
        let focus = Focus::Input;
        assert_eq!(focus.next(), Focus::Chat);
        assert_eq!(focus.next().next(), Focus::Sidebar);
        assert_eq!(focus.next().next().next(), Focus::Input);
    }

    #[test]
    fn test_app_new() {
        let app = App::new(None);
        assert!(app.running);
        assert_eq!(app.focus, Focus::Input);
        assert!(app.input.is_empty());
        assert!(app.connection_info.is_none());
        // Should have welcome message
        assert_eq!(app.messages.len(), 1);
    }

    #[test]
    fn test_app_add_message() {
        let mut app = App::new(None);
        app.add_message(ChatMessage::User("Hello".to_string()));
        assert_eq!(app.messages.len(), 2);
        app.add_message(ChatMessage::Assistant("Hi there!".to_string()));
        assert_eq!(app.messages.len(), 3);
    }

    #[test]
    fn test_app_clear_messages() {
        let mut app = App::new(None);
        app.add_message(ChatMessage::User("Hello".to_string()));
        app.clear_messages();
        assert!(app.messages.is_empty());
    }

    #[test]
    fn test_chat_message_type_label() {
        assert_eq!(ChatMessage::User("test".to_string()).type_label(), "You");
        assert_eq!(
            ChatMessage::Assistant("test".to_string()).type_label(),
            "Glance"
        );
        assert_eq!(ChatMessage::Error("test".to_string()).type_label(), "Error");
        assert_eq!(
            ChatMessage::System("test".to_string()).type_label(),
            "System"
        );
    }

    #[test]
    fn test_chat_scroll_sticky_behavior() {
        let mut app = App::new(None);

        // When at bottom (scroll=0), adding message keeps us at bottom
        app.chat_scroll = 0;
        app.add_message(ChatMessage::User("Hello".to_string()));
        assert_eq!(app.chat_scroll, 0);
        assert!(!app.has_new_messages);

        // When scrolled up, adding message sets has_new_messages flag
        app.chat_scroll = 5;
        app.add_message(ChatMessage::User("World".to_string()));
        assert_eq!(app.chat_scroll, 5); // Scroll position preserved
        assert!(app.has_new_messages);
    }

    #[test]
    fn test_chat_scroll_reset_on_clear() {
        let mut app = App::new(None);
        app.chat_scroll = 5;
        app.clear_messages();
        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn test_query_log_entry_success() {
        let entry = QueryLogEntry::success(
            "SELECT * FROM users".to_string(),
            Duration::from_millis(42),
            10,
        );
        assert_eq!(entry.status, QueryStatus::Success);
        assert_eq!(entry.row_count, Some(10));
        assert!(entry.error.is_none());
    }

    #[test]
    fn test_query_log_entry_error() {
        let entry = QueryLogEntry::error(
            "SELECT * FROM nonexistent".to_string(),
            Duration::from_millis(5),
            "relation does not exist".to_string(),
        );
        assert_eq!(entry.status, QueryStatus::Error);
        assert!(entry.row_count.is_none());
        assert_eq!(entry.error, Some("relation does not exist".to_string()));
    }

    #[test]
    fn test_query_log_entry_sql_preview() {
        let entry = QueryLogEntry::success(
            "SELECT id, name, email FROM users WHERE active = true".to_string(),
            Duration::from_millis(10),
            5,
        );
        // Should truncate at 30 chars
        assert_eq!(entry.sql_preview(30), "SELECT id, name, email FROM us");
        // Short preview
        assert_eq!(entry.sql_preview(10), "SELECT id,");
        // Full text if under limit
        let short_entry =
            QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(1), 1);
        assert_eq!(short_entry.sql_preview(30), "SELECT 1");
    }

    #[test]
    fn test_app_add_query_log() {
        let mut app = App::new(None);
        assert!(app.query_log.is_empty());

        let entry1 = QueryLogEntry::success("SELECT 1".to_string(), Duration::from_millis(10), 1);
        app.add_query_log(entry1);
        assert_eq!(app.query_log.len(), 1);

        let entry2 = QueryLogEntry::success("SELECT 2".to_string(), Duration::from_millis(20), 1);
        app.add_query_log(entry2);
        assert_eq!(app.query_log.len(), 2);
        // Most recent should be first
        assert_eq!(app.query_log[0].sql, "SELECT 2");
        assert_eq!(app.query_log[1].sql, "SELECT 1");
    }

    #[test]
    fn test_app_query_selection_navigation() {
        let mut app = App::new(None);

        // Add some queries
        app.add_query_log(QueryLogEntry::success(
            "Q1".to_string(),
            Duration::from_millis(1),
            1,
        ));
        app.add_query_log(QueryLogEntry::success(
            "Q2".to_string(),
            Duration::from_millis(1),
            1,
        ));
        app.add_query_log(QueryLogEntry::success(
            "Q3".to_string(),
            Duration::from_millis(1),
            1,
        ));

        // Initially no selection
        assert!(app.selected_query.is_none());

        // Select first (most recent)
        app.select_next_query();
        assert_eq!(app.selected_query, Some(0));

        // Move down
        app.select_next_query();
        assert_eq!(app.selected_query, Some(1));

        // Move up
        app.select_previous_query();
        assert_eq!(app.selected_query, Some(0));

        // Can't go above 0
        app.select_previous_query();
        assert_eq!(app.selected_query, Some(0));

        // Move to last
        app.select_next_query();
        app.select_next_query();
        assert_eq!(app.selected_query, Some(2));

        // Can't go past last
        app.select_next_query();
        assert_eq!(app.selected_query, Some(2));
    }

    #[test]
    fn test_app_query_detail_modal() {
        let mut app = App::new(None);
        app.add_query_log(QueryLogEntry::success(
            "SELECT 1".to_string(),
            Duration::from_millis(1),
            1,
        ));

        // Can't open modal without selection
        assert!(!app.show_query_detail);
        app.open_query_detail();
        assert!(!app.show_query_detail);

        // Select and open
        app.select_next_query();
        app.open_query_detail();
        assert!(app.show_query_detail);

        // Close
        app.close_query_detail();
        assert!(!app.show_query_detail);
    }

    #[test]
    fn test_app_selected_query_entry() {
        let mut app = App::new(None);
        app.add_query_log(QueryLogEntry::success(
            "SELECT 1".to_string(),
            Duration::from_millis(1),
            1,
        ));

        // No selection
        assert!(app.selected_query_entry().is_none());

        // With selection
        app.selected_query = Some(0);
        let entry = app.selected_query_entry();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().sql, "SELECT 1");
    }
}
