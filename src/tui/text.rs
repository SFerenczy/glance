//! Text manipulation utilities for TUI input handling.
//!
//! Provides pure functions for finding word boundaries in text,
//! enabling readline-style word deletion and future word movement features.

/// Find the start position of the word before the cursor.
///
/// Words are delimited by whitespace; punctuation is part of the word.
/// Returns 0 if cursor is at start or only whitespace precedes it.
///
/// # Algorithm
/// 1. Start at cursor position
/// 2. Skip any whitespace moving left
/// 3. Skip non-whitespace (the word) moving left
/// 4. Return position
pub fn find_word_start_backward(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut pos = cursor;

    // Skip whitespace moving left
    while pos > 0 && chars.get(pos - 1).is_some_and(|c| c.is_whitespace()) {
        pos -= 1;
    }

    // Skip non-whitespace (the word) moving left
    while pos > 0 && chars.get(pos - 1).is_some_and(|c| !c.is_whitespace()) {
        pos -= 1;
    }

    pos
}

/// Find the end position of the word after the cursor.
///
/// Words are delimited by whitespace; punctuation is part of the word.
/// Returns text length if cursor is at end or only whitespace follows.
///
/// # Algorithm
/// 1. Start at cursor position
/// 2. Skip any whitespace moving right
/// 3. Skip non-whitespace (the word) moving right
/// 4. Return position
pub fn find_word_end_forward(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut pos = cursor;

    // Skip whitespace moving right
    while pos < len && chars.get(pos).is_some_and(|c| c.is_whitespace()) {
        pos += 1;
    }

    // Skip non-whitespace (the word) moving right
    while pos < len && chars.get(pos).is_some_and(|c| !c.is_whitespace()) {
        pos += 1;
    }

    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    // Backward boundary tests

    #[test]
    fn find_word_start_at_end_of_word() {
        assert_eq!(find_word_start_backward("hello world", 11), 6);
    }

    #[test]
    fn find_word_start_with_trailing_spaces() {
        assert_eq!(find_word_start_backward("hello   ", 8), 0);
    }

    #[test]
    fn find_word_start_at_beginning() {
        assert_eq!(find_word_start_backward("hello", 0), 0);
    }

    #[test]
    fn find_word_start_mid_word() {
        assert_eq!(find_word_start_backward("hello", 3), 0);
    }

    #[test]
    fn find_word_start_with_punctuation() {
        assert_eq!(find_word_start_backward("hello, world!", 13), 7);
    }

    #[test]
    fn find_word_start_empty_string() {
        assert_eq!(find_word_start_backward("", 0), 0);
    }

    #[test]
    fn find_word_start_multiple_words() {
        // "one two three|" -> "one two |"
        assert_eq!(find_word_start_backward("one two three", 13), 8);
    }

    #[test]
    fn find_word_start_with_multiple_spaces() {
        // "one   two|" -> "one   |"
        assert_eq!(find_word_start_backward("one   two", 9), 6);
    }

    // Forward boundary tests

    #[test]
    fn find_word_end_at_start_of_word() {
        assert_eq!(find_word_end_forward("hello world", 0), 5);
    }

    #[test]
    fn find_word_end_with_leading_spaces() {
        assert_eq!(find_word_end_forward("   hello", 0), 8);
    }

    #[test]
    fn find_word_end_at_end() {
        assert_eq!(find_word_end_forward("hello", 5), 5);
    }

    #[test]
    fn find_word_end_empty_string() {
        assert_eq!(find_word_end_forward("", 0), 0);
    }

    #[test]
    fn find_word_end_mid_word() {
        // "hel|lo" -> "hello|"
        assert_eq!(find_word_end_forward("hello", 3), 5);
    }

    #[test]
    fn find_word_end_multiple_words() {
        // "|one two" -> "one| two"
        assert_eq!(find_word_end_forward("one two", 0), 3);
    }

    #[test]
    fn find_word_end_with_multiple_spaces() {
        // "one|   two" -> "one   two|"
        assert_eq!(find_word_end_forward("one   two", 3), 9);
    }

    // Tests from spec examples

    #[test]
    fn spec_example_backward_simple() {
        // "SELECT * FROM users|" -> "SELECT * FROM |"
        assert_eq!(find_word_start_backward("SELECT * FROM users", 19), 14);
    }

    #[test]
    fn spec_example_forward_simple() {
        // "SELECT * FROM |users" -> cursor at 14, delete to 19
        assert_eq!(find_word_end_forward("SELECT * FROM users", 14), 19);
    }

    // Unicode tests

    #[test]
    fn find_word_start_unicode() {
        // "hello \u4e16\u754c|" (hello world in Chinese)
        assert_eq!(find_word_start_backward("hello \u{4e16}\u{754c}", 8), 6);
    }

    #[test]
    fn find_word_end_unicode() {
        // "|hello \u4e16\u754c"
        assert_eq!(find_word_end_forward("hello \u{4e16}\u{754c}", 0), 5);
    }
}
