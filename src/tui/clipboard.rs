//! Clipboard support for the TUI.
//!
//! Provides cross-platform clipboard operations using arboard.

use arboard::Clipboard;
use std::sync::Mutex;

/// Global clipboard instance wrapped in a mutex for thread safety.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Initializes the clipboard. Should be called once at startup.
pub fn init() -> Result<(), ClipboardError> {
    let clipboard = Clipboard::new().map_err(|e| ClipboardError::Init(e.to_string()))?;
    let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
    *guard = Some(clipboard);
    Ok(())
}

/// Copies text to the clipboard.
pub fn copy(text: &str) -> Result<(), ClipboardError> {
    let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = guard.as_mut().ok_or(ClipboardError::NotInitialized)?;
    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::Copy(e.to_string()))
}

/// Gets text from the clipboard.
#[allow(dead_code)]
pub fn paste() -> Result<String, ClipboardError> {
    let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = guard.as_mut().ok_or(ClipboardError::NotInitialized)?;
    clipboard
        .get_text()
        .map_err(|e| ClipboardError::Paste(e.to_string()))
}

/// Clipboard operation errors.
#[derive(Debug, Clone)]
pub enum ClipboardError {
    /// Failed to initialize clipboard.
    Init(String),
    /// Failed to acquire lock.
    Lock,
    /// Clipboard not initialized.
    NotInitialized,
    /// Failed to copy to clipboard.
    Copy(String),
    /// Failed to paste from clipboard.
    Paste(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Init(e) => write!(f, "Failed to initialize clipboard: {}", e),
            Self::Lock => write!(f, "Failed to acquire clipboard lock"),
            Self::NotInitialized => write!(f, "Clipboard not initialized"),
            Self::Copy(e) => write!(f, "Failed to copy to clipboard: {}", e),
            Self::Paste(e) => write!(f, "Failed to paste from clipboard: {}", e),
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_error_display() {
        let err = ClipboardError::NotInitialized;
        assert_eq!(err.to_string(), "Clipboard not initialized");
    }
}
