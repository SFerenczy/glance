//! Command palette widget for the TUI.
//!
//! Provides a floating overlay showing available slash commands with fuzzy filtering.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// A slash command with its name and description.
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name (without the leading slash).
    pub name: &'static str,
    /// A brief description of what the command does.
    pub description: &'static str,
}

impl Command {
    /// Creates a new command.
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description }
    }
}

/// All available slash commands.
pub const COMMANDS: &[Command] = &[
    Command::new("sql", "Execute raw SQL directly"),
    Command::new("schema", "Display database schema"),
    Command::new("clear", "Clear chat history and LLM context"),
    Command::new("vim", "Toggle vim-style navigation mode"),
    Command::new("rownumbers", "Toggle row numbers in result tables"),
    Command::new("help", "Show help message"),
    Command::new("quit", "Exit the application"),
    Command::new("exit", "Exit the application"),
    // Connection management (v0.2b)
    Command::new("connections", "List saved database connections"),
    Command::new("connect", "Switch to a saved connection"),
    Command::new("conn add", "Add a new saved connection"),
    Command::new("conn edit", "Edit a saved connection"),
    Command::new("conn delete", "Delete a saved connection"),
    // Query history (v0.2b)
    Command::new("history", "Show query history"),
    Command::new("history clear", "Clear query history"),
    // Saved queries (v0.2b)
    Command::new("savequery", "Save current query with name and tags"),
    Command::new("queries", "List and search saved queries"),
    Command::new("usequery", "Load a saved query into input"),
    Command::new("query delete", "Delete a saved query"),
    // LLM configuration (v0.2b)
    Command::new("llm provider", "Set LLM provider (openai|anthropic|ollama)"),
    Command::new("llm key", "Set API key for current provider"),
    Command::new("llm model", "Set model for current provider"),
];

/// State for the command palette.
#[derive(Debug, Default)]
pub struct CommandPaletteState {
    /// Whether the palette is currently visible.
    pub visible: bool,
    /// Current filter text (after the `/`).
    pub filter: String,
    /// Currently selected index in the filtered results.
    pub selected: usize,
    /// Cached filtered results.
    filtered_commands: Vec<usize>,
    /// Flag indicating the input should be submitted after palette closes.
    pub submit_on_close: bool,
}

impl CommandPaletteState {
    /// Creates a new command palette state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the palette and resets state.
    pub fn open(&mut self) {
        self.visible = true;
        self.filter.clear();
        self.selected = 0;
        self.update_filtered();
    }

    /// Closes the palette.
    pub fn close(&mut self) {
        self.visible = false;
        self.filter.clear();
        self.selected = 0;
        self.filtered_commands.clear();
        // Note: submit_on_close is NOT cleared here - it's consumed by the event loop
    }

    /// Closes the palette and signals that input should be submitted.
    pub fn close_and_submit(&mut self) {
        self.submit_on_close = true;
        self.close();
    }

    /// Takes and clears the submit_on_close flag.
    pub fn take_submit_request(&mut self) -> bool {
        std::mem::take(&mut self.submit_on_close)
    }

    /// Updates the filter and refreshes results.
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.update_filtered();
        // Clamp selection to valid range
        if !self.filtered_commands.is_empty() {
            self.selected = self.selected.min(self.filtered_commands.len() - 1);
        } else {
            self.selected = 0;
        }
    }

    /// Moves selection up.
    pub fn select_previous(&mut self) {
        if !self.filtered_commands.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        if !self.filtered_commands.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered_commands.len() - 1);
        }
    }

    /// Returns the currently selected command, if any.
    pub fn selected_command(&self) -> Option<&'static Command> {
        self.filtered_commands
            .get(self.selected)
            .map(|&idx| &COMMANDS[idx])
    }

    /// Returns the filtered commands.
    pub fn filtered(&self) -> impl Iterator<Item = (usize, &'static Command)> + '_ {
        self.filtered_commands
            .iter()
            .enumerate()
            .map(|(display_idx, &cmd_idx)| (display_idx, &COMMANDS[cmd_idx]))
    }

    /// Updates the filtered commands based on current filter.
    fn update_filtered(&mut self) {
        self.filtered_commands.clear();

        if self.filter.is_empty() {
            // Show all commands when filter is empty (per v0.2a spec)
            self.filtered_commands.extend(0..COMMANDS.len());
        } else {
            // Score and sort commands by match quality
            let filter_lower = self.filter.to_lowercase();
            let mut scored: Vec<(usize, i32)> = COMMANDS
                .iter()
                .enumerate()
                .filter_map(|(idx, cmd)| {
                    let score = Self::match_score(cmd, &filter_lower);
                    if score > 0 {
                        Some((idx, score))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort by score (highest first)
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered_commands
                .extend(scored.into_iter().map(|(idx, _)| idx));
        }
    }

    /// Calculates a match score for a command against the filter.
    /// Higher scores are better matches.
    /// Returns 0 if no match.
    fn match_score(cmd: &Command, filter: &str) -> i32 {
        let name_lower = cmd.name.to_lowercase();
        let desc_lower = cmd.description.to_lowercase();

        // Exact prefix match on name (highest priority)
        if name_lower.starts_with(filter) {
            return 100;
        }

        // Prefix match on description words
        if desc_lower
            .split_whitespace()
            .any(|word| word.starts_with(filter))
        {
            return 50;
        }

        // Substring match on name
        if name_lower.contains(filter) {
            return 30;
        }

        // Substring match on description
        if desc_lower.contains(filter) {
            return 20;
        }

        // Fuzzy match on name
        if Self::fuzzy_match(&name_lower, filter) {
            return 10;
        }

        0
    }

    /// Simple fuzzy matching - checks if all filter chars appear in order.
    fn fuzzy_match(text: &str, filter: &str) -> bool {
        let mut text_chars = text.chars();
        for filter_char in filter.chars() {
            loop {
                match text_chars.next() {
                    Some(c) if c == filter_char => break,
                    Some(_) => continue,
                    None => return false,
                }
            }
        }
        true
    }
}

/// Command palette widget.
pub struct CommandPalette<'a> {
    state: &'a CommandPaletteState,
}

impl<'a> CommandPalette<'a> {
    /// Creates a new command palette widget.
    pub fn new(state: &'a CommandPaletteState) -> Self {
        Self { state }
    }

    /// Calculates the area for the palette popup.
    pub fn popup_area(input_area: Rect) -> Rect {
        // Position above the input bar
        let width = input_area.width.min(50);
        let height = (COMMANDS.len() as u16 + 2).min(10); // +2 for borders

        let x = input_area.x + 1; // Align with input content
        let y = input_area.y.saturating_sub(height);

        Rect::new(x, y, width, height)
    }
}

impl Widget for CommandPalette<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area first
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Commands ");

        let inner = block.inner(area);
        block.render(area, buf);

        // Render each command
        let mut y = inner.y;
        for (display_idx, cmd) in self.state.filtered() {
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = display_idx == self.state.selected;

            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let desc_style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Clear line with background if selected
            if is_selected {
                for x in inner.x..inner.x + inner.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(style);
                    }
                }
            }

            // Render command name and description
            let line = Line::from(vec![
                Span::styled(format!("/{}", cmd.name), name_style),
                Span::raw(" - "),
                Span::styled(cmd.description, desc_style),
            ]);

            let paragraph = Paragraph::new(line);
            let line_area = Rect::new(inner.x, y, inner.width, 1);
            paragraph.render(line_area, buf);

            y += 1;
        }

        // Show "no matches" if empty
        if self.state.filtered_commands.is_empty() {
            let no_match =
                Paragraph::new("No matching commands").style(Style::default().fg(Color::DarkGray));
            no_match.render(inner, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_open_close() {
        let mut state = CommandPaletteState::new();
        assert!(!state.visible);

        state.open();
        assert!(state.visible);
        assert!(state.filter.is_empty());

        state.close();
        assert!(!state.visible);
    }

    #[test]
    fn test_filter_empty_shows_all() {
        let mut state = CommandPaletteState::new();
        state.open();
        // Per v0.2a: Show all commands when filter is empty
        assert_eq!(state.filtered_commands.len(), COMMANDS.len());
    }

    #[test]
    fn test_filter_prefix_match() {
        let mut state = CommandPaletteState::new();
        state.open();
        state.set_filter("sq");

        // Should match "sql"
        assert!(!state.filtered_commands.is_empty());
        let cmd = state.selected_command().unwrap();
        assert_eq!(cmd.name, "sql");
    }

    #[test]
    fn test_filter_no_match() {
        let mut state = CommandPaletteState::new();
        state.open();
        state.set_filter("xyz");

        assert!(state.filtered_commands.is_empty());
        assert!(state.selected_command().is_none());
    }

    #[test]
    fn test_navigation() {
        let mut state = CommandPaletteState::new();
        state.open();
        // Need to set a filter to have items to navigate (empty filter shows nothing)
        state.set_filter("s");

        assert_eq!(state.selected, 0);
        assert!(!state.filtered_commands.is_empty());

        state.select_next();
        assert_eq!(state.selected, 1);

        state.select_previous();
        assert_eq!(state.selected, 0);

        // Can't go below 0
        state.select_previous();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_fuzzy_match() {
        assert!(CommandPaletteState::fuzzy_match("schema", "scm"));
        assert!(CommandPaletteState::fuzzy_match("clear", "clr"));
        assert!(!CommandPaletteState::fuzzy_match("sql", "xyz"));
    }
}
