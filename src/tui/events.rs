//! Event handling for the TUI.
//!
//! Processes keyboard and terminal events using crossterm.

use crate::error::{GlanceError, Result};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::Duration;

/// Application events.
#[derive(Debug)]
pub enum Event {
    /// A key was pressed.
    Key(KeyEvent),
    /// The terminal was resized.
    Resize(u16, u16),
    /// A periodic tick (for animations/updates).
    Tick,
}

/// Handles terminal events.
pub struct EventHandler {
    /// Timeout for polling events.
    tick_rate: Duration,
}

impl EventHandler {
    /// Creates a new event handler with default tick rate.
    pub fn new() -> Self {
        Self {
            tick_rate: Duration::from_millis(100),
        }
    }

    /// Creates a new event handler with a custom tick rate.
    #[allow(dead_code)]
    pub fn with_tick_rate(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    /// Polls for the next event.
    ///
    /// Returns `None` if no event is available within the tick rate.
    pub fn next(&self) -> Result<Option<Event>> {
        if event::poll(self.tick_rate)
            .map_err(|e| GlanceError::internal(format!("Failed to poll events: {e}")))?
        {
            let event = event::read()
                .map_err(|e| GlanceError::internal(format!("Failed to read event: {e}")))?;

            match event {
                CrosstermEvent::Key(key) => Ok(Some(Event::Key(key))),
                CrosstermEvent::Resize(width, height) => Ok(Some(Event::Resize(width, height))),
                _ => Ok(Some(Event::Tick)),
            }
        } else {
            Ok(Some(Event::Tick))
        }
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_handler_creation() {
        let handler = EventHandler::new();
        assert_eq!(handler.tick_rate, Duration::from_millis(100));
    }

    #[test]
    fn test_event_handler_custom_tick_rate() {
        let handler = EventHandler::with_tick_rate(Duration::from_millis(50));
        assert_eq!(handler.tick_rate, Duration::from_millis(50));
    }
}
