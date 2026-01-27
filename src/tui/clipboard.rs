//! Clipboard support for the TUI.
//!
//! Provides cross-platform clipboard operations with verification-based backend selection.
//!
//! Platform integration:
//! - Linux Wayland: wl-copy/wl-paste (preferred when WAYLAND_DISPLAY set)
//! - Linux X11: xclip or xsel (when DISPLAY set)
//! - macOS: pbcopy/pbpaste
//! - Windows: Native clipboard API (via arboard)
//! - Universal fallback: OSC 52 escape sequence (unverifiable)
//!
//! Backend selection uses write-read roundtrip verification to ensure the
//! clipboard actually works, rather than just checking if it can initialize.

use arboard::Clipboard;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Detected clipboard backend for the current platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ClipboardBackend {
    /// Linux Wayland: wl-copy/wl-paste commands.
    WlCopy,
    /// Native clipboard via arboard (Windows, some Linux/macOS).
    Arboard,
    /// Linux: xclip command.
    Xclip,
    /// Linux: xsel command.
    Xsel,
    /// macOS: pbcopy/pbpaste commands.
    Pbcopy,
    /// Terminal OSC 52 escape sequence (universal fallback, unverifiable).
    Osc52,
}

/// Result of a copy operation indicating verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyResult {
    /// Copy succeeded via a verified backend.
    Copied,
    /// Copy attempted via OSC 52 (cannot verify).
    CopiedUnverified,
}

/// Internal clipboard state.
struct ClipboardState {
    backend: Option<ClipboardBackend>,
    verified: bool,
    arboard_instance: Option<Clipboard>,
}

/// Global clipboard state.
static STATE: Mutex<ClipboardState> = Mutex::new(ClipboardState {
    backend: None,
    verified: false,
    arboard_instance: None,
});

/// Checks if a command exists by running it with --version.
fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Verifies a backend actually works via write-read roundtrip.
/// Returns false for OSC 52 since it cannot be verified.
fn verify_backend(backend: ClipboardBackend) -> bool {
    // OSC 52 cannot be verified - it writes to terminal with no feedback
    if backend == ClipboardBackend::Osc52 {
        return false;
    }

    let test_value = format!("glance-clipboard-test-{}", std::process::id());

    // Try to copy
    let copy_result = match backend {
        ClipboardBackend::WlCopy => copy_wlcopy(&test_value),
        ClipboardBackend::Arboard => copy_arboard_direct(&test_value),
        ClipboardBackend::Xclip => copy_xclip(&test_value),
        ClipboardBackend::Xsel => copy_xsel(&test_value),
        ClipboardBackend::Pbcopy => copy_pbcopy(&test_value),
        ClipboardBackend::Osc52 => return false,
    };

    if copy_result.is_err() {
        return false;
    }

    // Small delay for clipboard daemons to process
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Try to read back
    let paste_result = match backend {
        ClipboardBackend::WlCopy => paste_wlpaste(),
        ClipboardBackend::Arboard => paste_arboard_direct(),
        ClipboardBackend::Xclip => paste_xclip(),
        ClipboardBackend::Xsel => paste_xsel(),
        ClipboardBackend::Pbcopy => paste_pbpaste(),
        ClipboardBackend::Osc52 => return false,
    };

    paste_result.map(|v| v == test_value).unwrap_or(false)
}

/// Builds the list of candidate backends for Linux, ordered by preference.
#[cfg(target_os = "linux")]
fn build_linux_candidates() -> Vec<ClipboardBackend> {
    let mut candidates = Vec::new();

    // Wayland tools first if WAYLAND_DISPLAY is set
    if std::env::var("WAYLAND_DISPLAY").is_ok() && command_exists("wl-copy") {
        candidates.push(ClipboardBackend::WlCopy);
    }

    // X11 tools if DISPLAY is set
    if std::env::var("DISPLAY").is_ok() {
        if command_exists("xclip") {
            candidates.push(ClipboardBackend::Xclip);
        }
        if command_exists("xsel") {
            candidates.push(ClipboardBackend::Xsel);
        }
    }

    // Arboard as fallback (may work in some environments)
    candidates.push(ClipboardBackend::Arboard);

    candidates
}

/// Detects the best available clipboard backend for the current platform.
/// Uses verification to ensure the backend actually works.
fn detect_backend() -> Option<ClipboardBackend> {
    #[cfg(target_os = "linux")]
    {
        let candidates = build_linux_candidates();
        for backend in candidates {
            if verify_backend(backend) {
                return Some(backend);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS priority: pbcopy > arboard
        for backend in [ClipboardBackend::Pbcopy, ClipboardBackend::Arboard] {
            if verify_backend(backend) {
                return Some(backend);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: arboard only
        if verify_backend(ClipboardBackend::Arboard) {
            return Some(ClipboardBackend::Arboard);
        }
    }

    // OSC 52 is unverifiable - use as last resort
    Some(ClipboardBackend::Osc52)
}

/// Initializes the clipboard. Should be called once at startup.
pub fn init() -> Result<(), ClipboardError> {
    let mut state = STATE.lock().map_err(|_| ClipboardError::Lock)?;

    if let Some(backend) = detect_backend() {
        // Initialize arboard instance if needed
        if backend == ClipboardBackend::Arboard {
            state.arboard_instance = Clipboard::new().ok();
        }

        let verified = backend != ClipboardBackend::Osc52;
        state.backend = Some(backend);
        state.verified = verified;

        tracing::info!(
            "Clipboard initialized: {:?} (verified: {})",
            backend,
            verified
        );
    } else {
        tracing::warn!("No working clipboard backend found");
    }

    Ok(())
}

/// Returns the current clipboard backend.
#[allow(dead_code)]
pub fn backend() -> Option<ClipboardBackend> {
    STATE.lock().ok().and_then(|g| g.backend)
}

/// Copies text to the clipboard using the best available backend.
/// Returns `CopyResult::Copied` for verified backends, `CopyResult::CopiedUnverified` for OSC 52.
pub fn copy(text: &str) -> Result<CopyResult, ClipboardError> {
    let state = STATE.lock().map_err(|_| ClipboardError::Lock)?;

    let backend = state.backend.ok_or(ClipboardError::NotInitialized)?;
    let verified = state.verified;

    // Drop lock before calling copy functions
    drop(state);

    match backend {
        ClipboardBackend::WlCopy => copy_wlcopy(text)?,
        ClipboardBackend::Arboard => copy_arboard(text)?,
        ClipboardBackend::Xclip => copy_xclip(text)?,
        ClipboardBackend::Xsel => copy_xsel(text)?,
        ClipboardBackend::Pbcopy => copy_pbcopy(text)?,
        ClipboardBackend::Osc52 => copy_osc52(text)?,
    }

    Ok(if verified {
        CopyResult::Copied
    } else {
        CopyResult::CopiedUnverified
    })
}

/// Gets text from the clipboard using the best available backend.
#[allow(dead_code)]
pub fn paste() -> Result<String, ClipboardError> {
    let state = STATE.lock().map_err(|_| ClipboardError::Lock)?;
    let backend = state.backend.ok_or(ClipboardError::NotInitialized)?;
    drop(state);

    match backend {
        ClipboardBackend::WlCopy => paste_wlpaste(),
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

// --- wl-copy/wl-paste backend (Linux Wayland) ---

fn copy_wlcopy(text: &str) -> Result<(), ClipboardError> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ClipboardError::Copy(format!("Failed to spawn wl-copy: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::Copy(format!("Failed to write to wl-copy: {}", e)))?;
    }

    child
        .wait()
        .map_err(|e| ClipboardError::Copy(format!("wl-copy failed: {}", e)))?;

    Ok(())
}

fn paste_wlpaste() -> Result<String, ClipboardError> {
    let output = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .map_err(|e| ClipboardError::Paste(format!("Failed to run wl-paste: {}", e)))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ClipboardError::Paste(format!("Invalid UTF-8 from wl-paste: {}", e)))
    } else {
        Err(ClipboardError::Paste("wl-paste returned error".to_string()))
    }
}

// --- Arboard backend ---

/// Direct arboard copy for verification (creates temporary instance).
fn copy_arboard_direct(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::Copy(e.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::Copy(e.to_string()))
}

/// Direct arboard paste for verification (creates temporary instance).
fn paste_arboard_direct() -> Result<String, ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::Paste(e.to_string()))?;
    clipboard
        .get_text()
        .map_err(|e| ClipboardError::Paste(e.to_string()))
}

/// Arboard copy using the global instance.
fn copy_arboard(text: &str) -> Result<(), ClipboardError> {
    let mut state = STATE.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = state
        .arboard_instance
        .as_mut()
        .ok_or(ClipboardError::NotInitialized)?;
    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::Copy(e.to_string()))
}

/// Arboard paste using the global instance.
#[allow(dead_code)]
fn paste_arboard() -> Result<String, ClipboardError> {
    let mut state = STATE.lock().map_err(|_| ClipboardError::Lock)?;
    let clipboard = state
        .arboard_instance
        .as_mut()
        .ok_or(ClipboardError::NotInitialized)?;
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
/// Cannot be verified - always returns CopiedUnverified from copy().
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

    #[test]
    fn test_copy_result_variants() {
        let verified = CopyResult::Copied;
        let unverified = CopyResult::CopiedUnverified;

        assert!(matches!(verified, CopyResult::Copied));
        assert!(matches!(unverified, CopyResult::CopiedUnverified));
    }

    #[test]
    fn test_command_exists_for_nonexistent() {
        assert!(!command_exists("definitely-not-a-real-command-12345"));
    }
}
