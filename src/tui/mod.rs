//! Terminal User Interface for Glance.
//!
//! Provides the main TUI application loop using ratatui and crossterm.

mod app;
mod events;
mod ui;
pub mod widgets;

pub use app::App;
pub use events::{Event, EventHandler};

use crate::config::ConnectionConfig;
use crate::error::{GlanceError, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::panic;

/// The main TUI application runner.
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    event_handler: EventHandler,
}

impl Tui {
    /// Creates a new TUI instance, initializing the terminal.
    pub fn new() -> Result<Self> {
        let terminal = Self::setup_terminal()?;
        let event_handler = EventHandler::new();

        Ok(Self {
            terminal,
            event_handler,
        })
    }

    /// Sets up the terminal for TUI rendering.
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()
            .map_err(|e| GlanceError::internal(format!("Failed to enable raw mode: {e}")))?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| GlanceError::internal(format!("Failed to enter alternate screen: {e}")))?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)
            .map_err(|e| GlanceError::internal(format!("Failed to create terminal: {e}")))?;

        Ok(terminal)
    }

    /// Restores the terminal to its original state.
    fn restore_terminal(&mut self) -> Result<()> {
        disable_raw_mode()
            .map_err(|e| GlanceError::internal(format!("Failed to disable raw mode: {e}")))?;

        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(|e| GlanceError::internal(format!("Failed to leave alternate screen: {e}")))?;

        self.terminal
            .show_cursor()
            .map_err(|e| GlanceError::internal(format!("Failed to show cursor: {e}")))?;

        Ok(())
    }

    /// Runs the main TUI event loop.
    pub fn run(&mut self, connection: Option<&ConnectionConfig>) -> Result<()> {
        // Set up panic hook to restore terminal on panic
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            // Attempt to restore terminal
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            original_hook(panic_info);
        }));

        let mut app = App::new(connection);

        while app.running {
            // Draw the UI
            self.terminal
                .draw(|frame| ui::render(frame, &app))
                .map_err(|e| GlanceError::internal(format!("Failed to draw: {e}")))?;

            // Handle events
            if let Some(event) = self.event_handler.next()? {
                app.handle_event(event);
            }
        }

        // Restore panic hook
        let _ = panic::take_hook();

        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

/// Runs the TUI application.
pub fn run(connection: Option<&ConnectionConfig>) -> Result<()> {
    let mut tui = Tui::new()?;
    tui.run(connection)
}
