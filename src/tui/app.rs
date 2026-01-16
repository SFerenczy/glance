//! Application state for the TUI.
//!
//! Contains the main App struct and related types for managing UI state.

use crate::config::ConnectionConfig;

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

        Self {
            running: true,
            focus: Focus::default(),
            input: InputState::new(),
            chat_scroll: 0,
            sidebar_scroll: 0,
            connection_info,
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
    }
}
