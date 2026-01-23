//! Clipboard support for the TUI.
//!
//! Provides cross-platform clipboard operations with fallback support.
//!
//! Platform integration (per FR-4.3):
//! - Linux: Use `xclip` or `xsel` if available, fallback to OSC 52
//! - macOS: Use `pbcopy`/`pbpaste`
//! - Windows: Use native clipboard API (via arboard)
//! - Graceful fallback if clipboard unavailable

use arboard::Clipboard;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Global clipboard instance wrapped in a mutex for thread safety.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Detected clipboard backend for the current platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ClipboardBackend {
    /// Native clipboard via arboard (Windows, some Linux/macOS).
    Arboard,
    /// Linux: xclip command.
    Xclip,
    /// Linux: xsel command.
    Xsel,
    /// macOS: pbcopy/pbpaste commands.
    Pbcopy,
    /// Terminal OSC 52 escape sequence (universal fallback).
    Osc52,
}

/// Global backend selection.
static BACKEND: Mutex<Option<ClipboardBackend>> = Mutex::new(None);

/// Detects the best available clipboard backend for the current platform.
fn detect_backend() -> ClipboardBackend {
    // Try arboard first (works on Windows, and some Linux/macOS setups)
    if Clipboard::new().is_ok() {
        return ClipboardBackend::Arboard;
    }

    // On macOS, try pbcopy
    #[cfg(target_os = "macos")]
    {
        if Command::new("pbcopy")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return ClipboardBackend::Pbcopy;
        }
    }

    // On Linux, try xclip or xsel
    #[cfg(target_os = "linux")]
    {
        if Command::new("xclip")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return ClipboardBackend::Xclip;
        }
        if Command::new("xsel")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return ClipboardBackend::Xsel;
        }
    }

    // Fallback to OSC 52 (works in most modern terminals)
    ClipboardBackend::Osc52
}

/// Initializes the clipboard. Should be called once at startup.
pub fn init() -> Result<(), ClipboardError> {
    let backend = detect_backend();

    // Store the detected backend
    if let Ok(mut guard) = BACKEND.lock() {
        *guard = Some(backend);
    }

    // Initialize arboard if that's our backend
    if backend == ClipboardBackend::Arboard {
        let clipboard = Clipboard::new().map_err(|e| ClipboardError::Init(e.to_string()))?;
        let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
        *guard = Some(clipboard);
    }

    Ok(())
}

/// Returns the current clipboard backend.
pub fn backend() -> Option<ClipboardBackend> {
    BACKEND.lock().ok().and_then(|g| *g)
}

/// Copies text to the clipboard using the best available backend.
pub fn copy(text: &str) -> Result<(), ClipboardError> {
    let backend = backend().unwrap_or_else(detect_backend);

    match backend {
        ClipboardBackend::Arboard => copy_arboard(text),
        ClipboardBackend::Xclip => copy_xclip(text),
        ClipboardBackend::Xsel => copy_xsel(text),
        ClipboardBackend::Pbcopy => copy_pbcopy(text),
        ClipboardBackend::Osc52 => copy_osc52(text),
    }
}

/// Gets text from the clipboard using the best available backend.
#[allow(dead_code)]
pub fn paste() -> Result<String, ClipboardError> {
    let backend = backend().unwrap_or_else(detect_backend);

    match backend {
        ClipboardBackend::Arboard => paste_arboard(),
        ClipboardBackend::Xclip => paste_xclip(),
        ClipboardBackend::Xsel => paste_xsel(),
        ClipboardBackend::Pbcopy => paste_pbpaste(),
        ClipboardBackend::Osc52 => {
            // OSC 52 paste requires terminal cooperation and is not reliably supported
            Err(ClipboardError::Paste(
                "OSC 52 paste not supported; use terminal paste".to_string(),
            ))
        }
    }
}

// --- Arboard backend ---

fn copy_arboard(text: &str) -> Result<(), ClipboardError> {
    let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = guard.as_mut().ok_or(ClipboardError::NotInitialized)?;
    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::Copy(e.to_string()))
}

#[allow(dead_code)]
fn paste_arboard() -> Result<String, ClipboardError> {
    let mut guard = CLIPBOARD.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = guard.as_mut().ok_or(ClipboardError::NotInitialized)?;
    clipboard
        .get_text()
        .map_err(|e| ClipboardError::Paste(e.to_string()))
}

// --- xclip backend (Linux) ---

fn copy_xclip(text: &str) -> Result<(), ClipboardError> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ClipboardError::Copy(format!("Failed to spawn xclip: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::Copy(format!("Failed to write to xclip: {}", e)))?;
    }

    child
        .wait()
        .map_err(|e| ClipboardError::Copy(format!("xclip failed: {}", e)))?;

    Ok(())
}

#[allow(dead_code)]
fn paste_xclip() -> Result<String, ClipboardError> {
    let output = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
        .map_err(|e| ClipboardError::Paste(format!("Failed to run xclip: {}", e)))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ClipboardError::Paste(format!("Invalid UTF-8 from xclip: {}", e)))
    } else {
        Err(ClipboardError::Paste("xclip returned error".to_string()))
    }
}

// --- xsel backend (Linux) ---

fn copy_xsel(text: &str) -> Result<(), ClipboardError> {
    let mut child = Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ClipboardError::Copy(format!("Failed to spawn xsel: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::Copy(format!("Failed to write to xsel: {}", e)))?;
    }

    child
        .wait()
        .map_err(|e| ClipboardError::Copy(format!("xsel failed: {}", e)))?;

    Ok(())
}

#[allow(dead_code)]
fn paste_xsel() -> Result<String, ClipboardError> {
    let output = Command::new("xsel")
        .args(["--clipboard", "--output"])
        .output()
        .map_err(|e| ClipboardError::Paste(format!("Failed to run xsel: {}", e)))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ClipboardError::Paste(format!("Invalid UTF-8 from xsel: {}", e)))
    } else {
        Err(ClipboardError::Paste("xsel returned error".to_string()))
    }
}

// --- pbcopy/pbpaste backend (macOS) ---

fn copy_pbcopy(text: &str) -> Result<(), ClipboardError> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ClipboardError::Copy(format!("Failed to spawn pbcopy: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::Copy(format!("Failed to write to pbcopy: {}", e)))?;
    }

    child
        .wait()
        .map_err(|e| ClipboardError::Copy(format!("pbcopy failed: {}", e)))?;

    Ok(())
}

#[allow(dead_code)]
fn paste_pbpaste() -> Result<String, ClipboardError> {
    let output = Command::new("pbpaste")
        .output()
        .map_err(|e| ClipboardError::Paste(format!("Failed to run pbpaste: {}", e)))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ClipboardError::Paste(format!("Invalid UTF-8 from pbpaste: {}", e)))
    } else {
        Err(ClipboardError::Paste("pbpaste returned error".to_string()))
    }
}

// --- OSC 52 backend (universal terminal fallback) ---

/// Copies text using OSC 52 escape sequence.
/// This writes directly to stdout and works in most modern terminals.
fn copy_osc52(text: &str) -> Result<(), ClipboardError> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let encoded = STANDARD.encode(text);
    // OSC 52 format: ESC ] 52 ; c ; <base64-data> ESC \
    let sequence = format!("\x1b]52;c;{}\x1b\\", encoded);

    // Write to stdout (terminal)
    std::io::stdout()
        .write_all(sequence.as_bytes())
        .map_err(|e| ClipboardError::Copy(format!("Failed to write OSC 52: {}", e)))?;
    std::io::stdout()
        .flush()
        .map_err(|e| ClipboardError::Copy(format!("Failed to flush OSC 52: {}", e)))?;

    Ok(())
}

/// Clipboard operation errors.
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
