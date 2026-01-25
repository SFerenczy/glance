//! Input history for the TUI.
//!
//! Provides session-based input history with navigation support.

const MAX_HISTORY_SIZE: usize = 100;

/// Input history with circular buffer storage.
#[derive(Debug, Default)]
pub struct InputHistory {
    /// Stored history entries (oldest first).
    entries: Vec<String>,
    /// Current position in history (None = at newest/draft position).
    position: Option<usize>,
    /// Temporary storage for unsaved input when navigating history.
    draft: String,
}

impl InputHistory {
    /// Creates a new empty input history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry to the history.
    /// Skips empty entries and consecutive duplicates.
    pub fn push(&mut self, entry: String) {
        let entry = entry.trim().to_string();

        // Skip empty entries
        if entry.is_empty() {
            return;
        }

        // Skip consecutive duplicates
        if self.entries.last().map(|s| s.as_str()) == Some(&entry) {
            return;
        }

        // Add to history
        self.entries.push(entry);

        // Trim to max size (remove oldest)
        if self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.remove(0);
        }

        // Reset position after adding
        self.position = None;
        self.draft.clear();
    }

    /// Navigates to the previous (older) entry in history.
    /// Returns the entry to display, or None if at the oldest entry.
    pub fn previous(&mut self, current_input: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.position {
            None => {
                // Save current input as draft before navigating
                self.draft = current_input.to_string();
                // Move to most recent entry
                self.position = Some(self.entries.len() - 1);
            }
            Some(pos) if pos > 0 => {
                // Move to older entry
                self.position = Some(pos - 1);
            }
            Some(_) => {
                // Already at oldest entry
                return None;
            }
        }

        self.position.map(|pos| self.entries[pos].as_str())
    }

    /// Navigates to the next (newer) entry in history.
    /// Returns the entry to display, or the draft if returning to newest position.
    /// Returns empty string if already at newest to clear input.
    pub fn next(&mut self) -> Option<&str> {
        match self.position {
            None => {
                // Already at newest position - clear draft and return empty string
                self.draft.clear();
                Some("")
            }
            Some(pos) if pos + 1 < self.entries.len() => {
                // Move to newer entry
                self.position = Some(pos + 1);
                Some(self.entries[pos + 1].as_str())
            }
            Some(_) => {
                // Return to draft
                self.position = None;
                Some(self.draft.as_str())
            }
        }
    }

    /// Resets the navigation position without clearing history.
    pub fn reset_position(&mut self) {
        self.position = None;
        self.draft.clear();
    }

    /// Clears all history entries and resets state.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.position = None;
        self.draft.clear();
    }

    /// Returns the number of entries in history.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if history is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a reference to all history entries (oldest first).
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_push() {
        let mut history = InputHistory::new();
        history.push("first".to_string());
        history.push("second".to_string());
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_history_skip_empty() {
        let mut history = InputHistory::new();
        history.push("".to_string());
        history.push("   ".to_string());
        assert!(history.is_empty());
    }

    #[test]
    fn test_history_skip_consecutive_duplicates() {
        let mut history = InputHistory::new();
        history.push("same".to_string());
        history.push("same".to_string());
        history.push("same".to_string());
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_history_allows_non_consecutive_duplicates() {
        let mut history = InputHistory::new();
        history.push("first".to_string());
        history.push("second".to_string());
        history.push("first".to_string());
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_history_navigation() {
        let mut history = InputHistory::new();
        history.push("first".to_string());
        history.push("second".to_string());
        history.push("third".to_string());

        // Navigate backwards
        assert_eq!(history.previous("current"), Some("third"));
        assert_eq!(history.previous("current"), Some("second"));
        assert_eq!(history.previous("current"), Some("first"));
        assert_eq!(history.previous("current"), None); // At oldest

        // Navigate forwards
        assert_eq!(history.next(), Some("second"));
        assert_eq!(history.next(), Some("third"));
        assert_eq!(history.next(), Some("current")); // Back to draft
        assert_eq!(history.next(), Some("")); // Already at newest - clear input
    }

    #[test]
    fn test_history_preserves_draft() {
        let mut history = InputHistory::new();
        history.push("old".to_string());

        // Start with some typed input
        let draft = "my unsaved input";
        assert_eq!(history.previous(draft), Some("old"));

        // Return to draft
        assert_eq!(history.next(), Some(draft));
    }

    #[test]
    fn test_history_max_size() {
        let mut history = InputHistory::new();
        for i in 0..150 {
            history.push(format!("entry{}", i));
        }
        assert_eq!(history.len(), MAX_HISTORY_SIZE);

        // Oldest entries should be removed
        assert_eq!(history.previous(""), Some("entry149"));
    }

    #[test]
    fn test_history_reset_position() {
        let mut history = InputHistory::new();
        history.push("entry".to_string());

        history.previous("draft");
        assert!(history.position.is_some());

        history.reset_position();
        assert!(history.position.is_none());
    }

    #[test]
    fn test_empty_history_navigation() {
        let mut history = InputHistory::new();
        assert_eq!(history.previous("input"), None);
        assert_eq!(history.next(), Some("")); // Empty history - clear input
    }
}
