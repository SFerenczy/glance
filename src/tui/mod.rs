//! Terminal User Interface for Glance.
//!
//! Provides the main TUI application loop using ratatui and crossterm.

pub mod app;
mod clipboard;
mod events;
mod history;
mod sql_autocomplete;
mod ui;
pub mod widgets;

pub use app::App;
#[allow(unused_imports)]
pub use app::PendingQuery;
pub use events::{Event, EventHandler};

use crate::app::{InputResult, Orchestrator};
use crate::config::ConnectionConfig;
use crate::error::{GlanceError, Result};
use crate::llm::LlmProvider;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Messages sent from the async task to the main loop.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AsyncMessage {
    /// Input processing completed with a result.
    InputResult(Result<InputResult>),
    /// Query execution completed.
    QueryResult(Vec<app::ChatMessage>, Option<app::QueryLogEntry>),
}

/// The main TUI application runner.
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    event_handler: EventHandler,
    /// Flag to signal cancellation of pending operations.
    shutdown_flag: Arc<AtomicBool>,
}

impl Tui {
    /// Creates a new TUI instance, initializing the terminal.
    pub fn new() -> Result<Self> {
        let terminal = Self::setup_terminal()?;
        let event_handler = EventHandler::new();

        // Initialize clipboard (non-fatal if it fails)
        if let Err(e) = clipboard::init() {
            tracing::warn!("Failed to initialize clipboard: {}", e);
        }

        Ok(Self {
            terminal,
            event_handler,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Returns a clone of the shutdown flag for use in async tasks.
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown_flag)
    }

    /// Signals shutdown to all pending operations.
    pub fn signal_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }

    /// Checks if shutdown has been signaled.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
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

    /// Runs the main TUI event loop (synchronous version without orchestrator).
    pub fn run(&mut self, connection: Option<&ConnectionConfig>) -> Result<()> {
        // Set up panic hook to restore terminal on panic
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            // Attempt to restore terminal
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            original_hook(panic_info);
        }));

        let mut app_state = App::new(connection);

        while app_state.running {
            // Draw the UI
            self.terminal
                .draw(|frame| ui::render(frame, &app_state))
                .map_err(|e| GlanceError::internal(format!("Failed to draw: {e}")))?;

            // Handle events
            if let Some(event) = self.event_handler.next()? {
                app_state.handle_event(event);
            }
        }

        // Restore panic hook
        let _ = panic::take_hook();

        Ok(())
    }

    /// Runs the main TUI event loop with orchestrator (async version).
    pub async fn run_with_orchestrator(
        &mut self,
        connection: Option<&ConnectionConfig>,
        mut orchestrator: Orchestrator,
    ) -> Result<()> {
        // Set up panic hook to restore terminal on panic
        let original_hook = panic::take_hook();
        let shutdown_flag = self.shutdown_flag();
        panic::set_hook(Box::new(move |panic_info| {
            // Signal shutdown to cancel any pending operations
            shutdown_flag.store(true, Ordering::SeqCst);
            // Restore terminal state
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            original_hook(panic_info);
        }));

        let mut app_state = App::new(connection);

        // Channel for async messages
        let (tx, mut rx) = mpsc::channel::<AsyncMessage>(32);

        let result = self
            .run_event_loop(&mut app_state, &mut orchestrator, tx, &mut rx)
            .await;

        // Cleanup: signal shutdown and close resources
        self.signal_shutdown();

        // Close database connection gracefully
        if let Err(e) = orchestrator.close().await {
            warn!("Error closing database connection: {}", e);
        }

        // Restore panic hook
        let _ = panic::take_hook();

        result
    }

    /// The main event loop, separated for cleaner error handling.
    async fn run_event_loop(
        &mut self,
        app_state: &mut App,
        orchestrator: &mut Orchestrator,
        tx: mpsc::Sender<AsyncMessage>,
        rx: &mut mpsc::Receiver<AsyncMessage>,
    ) -> Result<()> {
        loop {
            // Clear expired toast notifications
            app_state.clear_expired_toast();

            // Ring terminal bell if requested (for long query notification)
            if app_state.take_bell_request() {
                print!("\x07"); // ASCII BEL character
            }

            // Draw the UI
            self.terminal
                .draw(|frame| ui::render(frame, app_state))
                .map_err(|e| GlanceError::internal(format!("Failed to draw: {e}")))?;

            if !app_state.running || self.is_shutdown() {
                break;
            }

            // Use tokio::select! to handle both events and async messages
            tokio::select! {
                // Handle terminal events
                event_result = tokio::task::spawn_blocking({
                    let tick_rate = std::time::Duration::from_millis(100);
                    move || {
                        if crossterm::event::poll(tick_rate).unwrap_or(false) {
                            crossterm::event::read().ok()
                        } else {
                            None
                        }
                    }
                }) => {
                    if let Ok(Some(event)) = event_result {
                        self.handle_crossterm_event(
                            event,
                            app_state,
                            orchestrator,
                            tx.clone(),
                        ).await;
                    }
                }

                // Handle async messages from background tasks
                Some(msg) = rx.recv() => {
                    self.handle_async_message(msg, app_state);
                }
            }
        }

        Ok(())
    }

    /// Handles a crossterm event.
    async fn handle_crossterm_event(
        &mut self,
        event: crossterm::event::Event,
        app_state: &mut App,
        orchestrator: &mut Orchestrator,
        _tx: mpsc::Sender<AsyncMessage>,
    ) {
        use crossterm::event::Event as CEvent;

        match event {
            CEvent::Key(key) => {
                // Handle confirmation dialog first
                if app_state.has_pending_query() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            // Confirm the query
                            if let Some(pending) = app_state.take_pending_query() {
                                app_state.is_processing = true;
                                let messages = orchestrator.confirm_query(&pending.sql).await;
                                for msg in messages {
                                    if let app::ChatMessage::Result(ref result) = msg {
                                        let entry = app::QueryLogEntry::success(
                                            pending.sql.clone(),
                                            result.execution_time,
                                            result.row_count,
                                        );
                                        app_state.add_query_log(entry);
                                    }
                                    app_state.add_message(msg);
                                }
                                app_state.is_processing = false;
                            }
                            return;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            // Cancel the query
                            app_state.clear_pending_query();
                            app_state.add_message(orchestrator.cancel_query());
                            return;
                        }
                        _ => return, // Ignore other keys when dialog is shown
                    }
                }

                // Handle global shortcuts
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_state.running = false;
                        return;
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_state.running = false;
                        return;
                    }
                    KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_state.clear_messages();
                        return;
                    }
                    _ => {}
                }

                // Handle input submission
                if key.code == KeyCode::Enter && app_state.focus == app::Focus::Input {
                    if let Some(input) = app_state.submit_input() {
                        // Add user message to chat
                        app_state.add_message(app::ChatMessage::User(input.clone()));
                        app_state.is_processing = true;

                        // Process input through orchestrator
                        match orchestrator.handle_input(&input).await {
                            Ok(result) => {
                                self.handle_input_result(result, app_state, orchestrator)
                                    .await;
                            }
                            Err(e) => {
                                error!("Error processing input: {}", e);
                                app_state.add_message(app::ChatMessage::Error(e.to_string()));
                            }
                        }
                        app_state.is_processing = false;
                    }
                    return;
                }

                // Convert to our Event type and handle normally
                let our_event = Event::Key(key);
                app_state.handle_event(our_event);
            }
            CEvent::Resize(w, h) => {
                app_state.handle_event(Event::Resize(w, h));
            }
            _ => {}
        }
    }

    /// Handles the result of processing user input.
    async fn handle_input_result(
        &mut self,
        result: InputResult,
        app_state: &mut App,
        _orchestrator: &mut Orchestrator,
    ) {
        match result {
            InputResult::None => {}
            InputResult::Messages(messages) => {
                for msg in messages {
                    // Track query log entries for results
                    if let app::ChatMessage::Result(ref query_result) = msg {
                        // The query log entry should be added by the orchestrator
                        debug!("Query returned {} rows", query_result.row_count);
                    }
                    app_state.add_message(msg);
                }
            }
            InputResult::NeedsConfirmation {
                sql,
                classification,
            } => {
                app_state.set_pending_query(sql, classification);
            }
            InputResult::Exit => {
                app_state.running = false;
            }
            InputResult::ToggleVimMode => {
                app_state.toggle_vim_mode();
            }
        }
    }

    /// Handles an async message from a background task.
    fn handle_async_message(&mut self, msg: AsyncMessage, app_state: &mut App) {
        match msg {
            AsyncMessage::InputResult(result) => {
                app_state.is_processing = false;
                match result {
                    Ok(InputResult::Messages(messages)) => {
                        for m in messages {
                            app_state.add_message(m);
                        }
                    }
                    Ok(InputResult::NeedsConfirmation {
                        sql,
                        classification,
                    }) => {
                        app_state.set_pending_query(sql, classification);
                    }
                    Ok(InputResult::Exit) => {
                        app_state.running = false;
                    }
                    Ok(InputResult::ToggleVimMode) => {
                        app_state.toggle_vim_mode();
                    }
                    Ok(InputResult::None) => {}
                    Err(e) => {
                        app_state.add_message(app::ChatMessage::Error(e.to_string()));
                    }
                }
            }
            AsyncMessage::QueryResult(messages, entry) => {
                for m in messages {
                    app_state.add_message(m);
                }
                if let Some(e) = entry {
                    app_state.add_query_log(e);
                }
            }
        }
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

/// Runs the TUI application (synchronous, without orchestrator).
pub fn run(connection: Option<&ConnectionConfig>) -> Result<()> {
    let mut tui = Tui::new()?;
    tui.run(connection)
}

/// Runs the TUI application with full orchestrator integration.
pub async fn run_async(connection: &ConnectionConfig, llm_provider: LlmProvider) -> Result<()> {
    info!("Connecting to database...");
    let orchestrator = Orchestrator::connect(connection, llm_provider).await?;
    info!("Connected successfully");

    let mut tui = Tui::new()?;
    tui.run_with_orchestrator(Some(connection), orchestrator)
        .await
}
