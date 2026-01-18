//! Event DSL parser for headless mode.
//!
//! Parses event strings like "key:enter", "type:hello", "wait:100ms" into
//! executable events.

use crate::error::{GlanceError, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fmt;
use std::time::Duration;

/// An assertion to check against the screen or state.
#[derive(Debug, Clone)]
pub enum Assertion {
    /// Screen contains text (case-insensitive).
    Contains(String),
    /// Screen contains text (case-sensitive).
    ContainsExact(String),
    /// Screen does not contain text.
    NotContains(String),
    /// Screen matches regex pattern.
    Matches(String),
    /// State field equals value.
    StateEquals { field: String, value: String },
    /// State field comparison (>=, <=, >, <).
    StateCompare {
        field: String,
        op: String,
        value: String,
    },
}

impl Assertion {
    /// Checks the assertion against the screen and app state.
    pub fn check(&self, screen: &str, app: &crate::tui::app::App) -> bool {
        match self {
            Self::Contains(text) => screen.to_lowercase().contains(&text.to_lowercase()),
            Self::ContainsExact(text) => screen.contains(text),
            Self::NotContains(text) => !screen.to_lowercase().contains(&text.to_lowercase()),
            Self::Matches(pattern) => regex::Regex::new(pattern)
                .map(|re| re.is_match(screen))
                .unwrap_or(false),
            Self::StateEquals { field, value } => {
                let actual = get_state_field(app, field);
                actual.as_deref() == Some(value.as_str())
            }
            Self::StateCompare { field, op, value } => {
                let actual = get_state_field(app, field);
                compare_values(actual.as_deref(), op, value)
            }
        }
    }
}

/// Gets a state field value from the app.
fn get_state_field(app: &crate::tui::app::App, field: &str) -> Option<String> {
    match field {
        "focus" => Some(format!("{:?}", app.focus)),
        "input_text" => Some(app.input.text.clone()),
        "is_processing" => Some(app.is_processing.to_string()),
        "running" => Some(app.running.to_string()),
        "message_count" => Some(app.messages.len().to_string()),
        _ => None,
    }
}

/// Compares values using the given operator.
fn compare_values(actual: Option<&str>, op: &str, expected: &str) -> bool {
    let actual = match actual {
        Some(v) => v,
        None => return false,
    };

    // Try numeric comparison first
    if let (Ok(a), Ok(e)) = (actual.parse::<i64>(), expected.parse::<i64>()) {
        return match op {
            ">=" => a >= e,
            "<=" => a <= e,
            ">" => a > e,
            "<" => a < e,
            "=" | "==" => a == e,
            _ => false,
        };
    }

    // Fall back to string comparison
    match op {
        "=" | "==" => actual == expected,
        _ => false,
    }
}

/// A parsed event that can be executed.
#[derive(Debug, Clone)]
pub enum Event {
    /// A key press event.
    Key(KeyEvent),
    /// Type text (expands to multiple key events).
    Type(String),
    /// Wait for a duration.
    Wait(Duration),
    /// Resize the terminal.
    Resize(u16, u16),
    /// Take a named snapshot.
    Snapshot(String),
    /// Assert something about the screen or state.
    Assert(Assertion),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => {
                let mut parts = Vec::new();
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    parts.push("ctrl");
                }
                if key.modifiers.contains(KeyModifiers::ALT) {
                    parts.push("alt");
                }
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    parts.push("shift");
                }
                let key_name = key_code_to_string(&key.code);
                parts.push(&key_name);
                write!(f, "key:{}", parts.join("+"))
            }
            Self::Type(text) => write!(f, "type:{}", text),
            Self::Wait(d) => write!(f, "wait:{}ms", d.as_millis()),
            Self::Resize(w, h) => write!(f, "resize:{}x{}", w, h),
            Self::Snapshot(name) => write!(f, "snapshot:{}", name),
            Self::Assert(a) => match a {
                Assertion::Contains(t) => write!(f, "assert:contains:{}", t),
                Assertion::ContainsExact(t) => write!(f, "assert:contains-exact:{}", t),
                Assertion::NotContains(t) => write!(f, "assert:not-contains:{}", t),
                Assertion::Matches(p) => write!(f, "assert:matches:{}", p),
                Assertion::StateEquals { field, value } => {
                    write!(f, "assert:state:{}={}", field, value)
                }
                Assertion::StateCompare { field, op, value } => {
                    write!(f, "assert:state:{}{}{}", field, op, value)
                }
            },
        }
    }
}

fn key_code_to_string(code: &KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::F(n) => format!("f{}", n),
        _ => "unknown".to_string(),
    }
}

/// Parser for the event DSL.
#[derive(Debug, Default)]
pub struct EventParser;

impl EventParser {
    /// Creates a new event parser.
    pub fn new() -> Self {
        Self
    }

    /// Parses all events from an input string.
    /// Supports comma-separated and newline-separated events.
    pub fn parse_all(&self, input: &str) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        for line in input.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split by comma for inline events
            for part in line.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }

                events.push(self.parse_one(part)?);
            }
        }

        Ok(events)
    }

    /// Parses a single event string.
    pub fn parse_one(&self, input: &str) -> Result<Event> {
        let input = input.trim();

        // Split into type and value
        let (event_type, value) = match input.split_once(':') {
            Some((t, v)) => (t.trim().to_lowercase(), v.trim()),
            None => {
                return Err(GlanceError::config(format!(
                    "Invalid event syntax: '{}'. Expected format: type:value",
                    input
                )));
            }
        };

        match event_type.as_str() {
            "key" => self.parse_key(value),
            "type" => Ok(Event::Type(value.to_string())),
            "wait" => self.parse_wait(value),
            "resize" => self.parse_resize(value),
            "snapshot" => Ok(Event::Snapshot(value.to_string())),
            "assert" => self.parse_assert(value),
            _ => Err(GlanceError::config(format!(
                "Unknown event type: '{}'. Valid types: key, type, wait, resize, snapshot, assert",
                event_type
            ))),
        }
    }

    /// Parses a key event like "enter", "ctrl+c", "shift+tab".
    fn parse_key(&self, value: &str) -> Result<Event> {
        let parts: Vec<&str> = value.split('+').collect();
        let mut modifiers = KeyModifiers::empty();
        let mut key_str = "";

        for (i, part) in parts.iter().enumerate() {
            let part_lower = part.to_lowercase();
            if i == parts.len() - 1 {
                // Last part is the key
                key_str = part;
            } else {
                // Modifier
                match part_lower.as_str() {
                    "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    _ => {
                        return Err(GlanceError::config(format!(
                            "Unknown modifier: '{}'. Valid modifiers: ctrl, alt, shift",
                            part
                        )));
                    }
                }
            }
        }

        let code = self.parse_key_code(key_str)?;

        Ok(Event::Key(KeyEvent::new(code, modifiers)))
    }

    /// Parses a key code string into a KeyCode.
    fn parse_key_code(&self, s: &str) -> Result<KeyCode> {
        let s_lower = s.to_lowercase();

        // Check for function keys first
        if s_lower.starts_with('f') && s_lower.len() > 1 {
            if let Ok(n) = s_lower[1..].parse::<u8>() {
                if (1..=12).contains(&n) {
                    return Ok(KeyCode::F(n));
                }
            }
        }

        let code = match s_lower.as_str() {
            "enter" | "return" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backspace" | "bs" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" | "pgup" => KeyCode::PageUp,
            "pagedown" | "pgdn" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            _ => {
                // Single character
                let chars: Vec<char> = s.chars().collect();
                if chars.len() == 1 {
                    KeyCode::Char(chars[0])
                } else {
                    return Err(GlanceError::config(format!(
                        "Unknown key: '{}'. Use single characters or named keys like enter, esc, tab, etc.",
                        s
                    )));
                }
            }
        };

        Ok(code)
    }

    /// Parses a wait duration like "100ms", "2s", or just "100" (defaults to ms).
    fn parse_wait(&self, value: &str) -> Result<Event> {
        let value = value.trim().to_lowercase();

        let duration = if value.ends_with("ms") {
            let num: u64 = value[..value.len() - 2]
                .parse()
                .map_err(|_| GlanceError::config(format!("Invalid duration: '{}'", value)))?;
            Duration::from_millis(num)
        } else if value.ends_with('s') {
            let num: u64 = value[..value.len() - 1]
                .parse()
                .map_err(|_| GlanceError::config(format!("Invalid duration: '{}'", value)))?;
            Duration::from_secs(num)
        } else {
            // Default to milliseconds
            let num: u64 = value
                .parse()
                .map_err(|_| GlanceError::config(format!("Invalid duration: '{}'", value)))?;
            Duration::from_millis(num)
        };

        Ok(Event::Wait(duration))
    }

    /// Parses a resize event like "120x40".
    fn parse_resize(&self, value: &str) -> Result<Event> {
        let parts: Vec<&str> = value.split('x').collect();
        if parts.len() != 2 {
            return Err(GlanceError::config(format!(
                "Invalid resize format: '{}'. Expected WIDTHxHEIGHT",
                value
            )));
        }

        let width: u16 = parts[0]
            .parse()
            .map_err(|_| GlanceError::config(format!("Invalid width: '{}'", parts[0])))?;
        let height: u16 = parts[1]
            .parse()
            .map_err(|_| GlanceError::config(format!("Invalid height: '{}'", parts[1])))?;

        Ok(Event::Resize(width, height))
    }

    /// Parses an assertion like "contains:hello" or "state:focus=Input".
    fn parse_assert(&self, value: &str) -> Result<Event> {
        let (assert_type, rest) = match value.split_once(':') {
            Some((t, r)) => (t.trim().to_lowercase(), r.trim()),
            None => {
                return Err(GlanceError::config(format!(
                    "Invalid assertion syntax: '{}'. Expected assert:type:value",
                    value
                )));
            }
        };

        let assertion = match assert_type.as_str() {
            "contains" => Assertion::Contains(rest.to_string()),
            "contains-exact" => Assertion::ContainsExact(rest.to_string()),
            "not-contains" => Assertion::NotContains(rest.to_string()),
            "matches" => Assertion::Matches(rest.to_string()),
            "state" => self.parse_state_assertion(rest)?,
            _ => {
                return Err(GlanceError::config(format!(
                    "Unknown assertion type: '{}'. Valid types: contains, contains-exact, not-contains, matches, state",
                    assert_type
                )));
            }
        };

        Ok(Event::Assert(assertion))
    }

    /// Parses a state assertion like "focus=Input" or "message_count>=2".
    fn parse_state_assertion(&self, value: &str) -> Result<Assertion> {
        // Try to find comparison operators
        for op in &[">=", "<=", ">", "<", "="] {
            if let Some(pos) = value.find(op) {
                let field = value[..pos].trim().to_string();
                let val = value[pos + op.len()..].trim().to_string();

                if *op == "=" {
                    return Ok(Assertion::StateEquals { field, value: val });
                } else {
                    return Ok(Assertion::StateCompare {
                        field,
                        op: op.to_string(),
                        value: val,
                    });
                }
            }
        }

        Err(GlanceError::config(format!(
            "Invalid state assertion: '{}'. Expected field=value or field>=value",
            value
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_enter() {
        let parser = EventParser::new();
        let event = parser.parse_one("key:enter").unwrap();
        match event {
            Event::Key(key) => {
                assert_eq!(key.code, KeyCode::Enter);
                assert!(key.modifiers.is_empty());
            }
            _ => panic!("Expected Key event"),
        }
    }

    #[test]
    fn test_parse_key_with_modifier() {
        let parser = EventParser::new();
        let event = parser.parse_one("key:ctrl+c").unwrap();
        match event {
            Event::Key(key) => {
                assert_eq!(key.code, KeyCode::Char('c'));
                assert!(key.modifiers.contains(KeyModifiers::CONTROL));
            }
            _ => panic!("Expected Key event"),
        }
    }

    #[test]
    fn test_parse_type() {
        let parser = EventParser::new();
        let event = parser.parse_one("type:hello world").unwrap();
        match event {
            Event::Type(text) => assert_eq!(text, "hello world"),
            _ => panic!("Expected Type event"),
        }
    }

    #[test]
    fn test_parse_wait_ms() {
        let parser = EventParser::new();
        let event = parser.parse_one("wait:100ms").unwrap();
        match event {
            Event::Wait(d) => assert_eq!(d, Duration::from_millis(100)),
            _ => panic!("Expected Wait event"),
        }
    }

    #[test]
    fn test_parse_wait_seconds() {
        let parser = EventParser::new();
        let event = parser.parse_one("wait:2s").unwrap();
        match event {
            Event::Wait(d) => assert_eq!(d, Duration::from_secs(2)),
            _ => panic!("Expected Wait event"),
        }
    }

    #[test]
    fn test_parse_wait_default_ms() {
        let parser = EventParser::new();
        let event = parser.parse_one("wait:100").unwrap();
        match event {
            Event::Wait(d) => assert_eq!(d, Duration::from_millis(100)),
            _ => panic!("Expected Wait event"),
        }
    }

    #[test]
    fn test_parse_resize() {
        let parser = EventParser::new();
        let event = parser.parse_one("resize:120x40").unwrap();
        match event {
            Event::Resize(w, h) => {
                assert_eq!(w, 120);
                assert_eq!(h, 40);
            }
            _ => panic!("Expected Resize event"),
        }
    }

    #[test]
    fn test_parse_comma_separated() {
        let parser = EventParser::new();
        let events = parser.parse_all("type:hello,key:enter,wait:100").unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_parse_with_comments() {
        let parser = EventParser::new();
        let script = r#"
# This is a comment
type:hello

# Another comment
key:enter
"#;
        let events = parser.parse_all(script).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_parse_assert_contains() {
        let parser = EventParser::new();
        let event = parser.parse_one("assert:contains:hello").unwrap();
        match event {
            Event::Assert(Assertion::Contains(text)) => assert_eq!(text, "hello"),
            _ => panic!("Expected Assert Contains event"),
        }
    }

    #[test]
    fn test_parse_assert_state() {
        let parser = EventParser::new();
        let event = parser.parse_one("assert:state:focus=Input").unwrap();
        match event {
            Event::Assert(Assertion::StateEquals { field, value }) => {
                assert_eq!(field, "focus");
                assert_eq!(value, "Input");
            }
            _ => panic!("Expected Assert StateEquals event"),
        }
    }

    #[test]
    fn test_parse_assert_state_compare() {
        let parser = EventParser::new();
        let event = parser.parse_one("assert:state:message_count>=2").unwrap();
        match event {
            Event::Assert(Assertion::StateCompare { field, op, value }) => {
                assert_eq!(field, "message_count");
                assert_eq!(op, ">=");
                assert_eq!(value, "2");
            }
            _ => panic!("Expected Assert StateCompare event"),
        }
    }

    #[test]
    fn test_parse_function_keys() {
        let parser = EventParser::new();
        for n in 1..=12 {
            let event = parser.parse_one(&format!("key:f{}", n)).unwrap();
            match event {
                Event::Key(key) => assert_eq!(key.code, KeyCode::F(n)),
                _ => panic!("Expected Key event"),
            }
        }
    }

    #[test]
    fn test_parse_invalid_event() {
        let parser = EventParser::new();
        assert!(parser.parse_one("invalid:event").is_err());
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let parser = EventParser::new();
        assert!(parser.parse_one("no_colon").is_err());
    }
}
