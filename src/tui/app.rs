//! Application state for the TUI.
//!
//! Contains the main App struct and related types for managing UI state.

use crate::config::ConnectionConfig;
use crate::db::QueryResult;

/// Which panel currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Input,
    Chat,
    Sidebar,
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
#[allow(dead_code)]
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
}

/// Main application state.
pub struct App {
    /// Whether the application is still running.
    pub running: bool,
    /// Current focus panel.
    pub focus: Focus,
    /// Input field state.
    pub input: InputState,
    /// Chat messages.
    pub messages: Vec<ChatMessage>,
    /// Chat scroll offset (lines from bottom).
    pub chat_scroll: usize,
    /// Sidebar scroll offset.
    pub sidebar_scroll: usize,
    /// Database connection info for display.
    pub connection_info: Option<String>,
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
            input: InputState::new(),
            messages,
            chat_scroll: 0,
            sidebar_scroll: 0,
            connection_info,
        }
    }

    /// Adds a message to the chat.
    #[allow(dead_code)]
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        // Auto-scroll to bottom when new message is added
        self.chat_scroll = 0;
    }

    /// Clears all chat messages.
    #[allow(dead_code)]
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.chat_scroll = 0;
    }

    /// Returns the total number of lines needed to render all messages.
    /// This is used for scroll calculations.
    #[allow(dead_code)]
    pub fn total_chat_lines(&self) -> usize {
        self.messages
            .iter()
            .map(Self::message_line_count)
            .sum()
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
                    }
                    KeyCode::PageUp if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_add(10);
                    }
                    KeyCode::PageDown if self.focus == Focus::Chat => {
                        self.chat_scroll = self.chat_scroll.saturating_sub(10);
                    }
                    KeyCode::Home if self.focus == Focus::Chat => {
                        self.chat_scroll = usize::MAX; // Will be clamped during render
                    }
                    KeyCode::End if self.focus == Focus::Chat => {
                        self.chat_scroll = 0;
                    }

                    // Sidebar scrolling (when sidebar is focused)
                    KeyCode::Up if self.focus == Focus::Sidebar => {
                        self.sidebar_scroll = self.sidebar_scroll.saturating_add(1);
                    }
                    KeyCode::Down if self.focus == Focus::Sidebar => {
                        self.sidebar_scroll = self.sidebar_scroll.saturating_sub(1);
                    }

                    _ => {}
                }
            }
            Event::Resize(_, _) => {
                // Terminal resize is handled automatically by ratatui
            }
            Event::Tick => {
                // Periodic tick for animations/updates (not used yet)
            }
        }
    }

    /// Handles key events when input is focused.
    fn handle_input_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char(c) => {
                self.input.insert(c);
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
                let text = self.input.take();
                if !text.is_empty() {
                    // TODO: Process the input (send to orchestrator)
                    // For now, just clear it
                    let _ = text;
                }
            }
            _ => {}
        }
    }

    /// Submits the current input for processing.
    #[allow(dead_code)]
    pub fn submit_input(&mut self) -> Option<String> {
        if self.input.is_empty() {
            None
        } else {
            Some(self.input.take())
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
    fn test_chat_scroll_reset_on_new_message() {
        let mut app = App::new(None);
        app.chat_scroll = 5;
        app.add_message(ChatMessage::User("Hello".to_string()));
        // Scroll should reset to 0 (bottom) when new message added
        assert_eq!(app.chat_scroll, 0);
    }

    #[test]
    fn test_chat_scroll_reset_on_clear() {
        let mut app = App::new(None);
        app.chat_scroll = 5;
        app.clear_messages();
        assert_eq!(app.chat_scroll, 0);
    }
}
