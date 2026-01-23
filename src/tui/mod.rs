//! Terminal User Interface for Glance.
//!
//! Provides the main TUI application loop using ratatui and crossterm.

pub mod app;
mod clipboard;
mod events;
pub mod headless;
mod history;
pub mod orchestrator_actor;
pub mod output_adapter;
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
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use orchestrator_actor::{OrchestratorActor, OrchestratorHandle, OrchestratorResponse, RequestId};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Messages sent from the async task to the main loop.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AsyncMessage {
    /// Input processing completed with a result.
    InputResult(Result<InputResult>),
    /// Query execution completed.
    QueryResult(Vec<app::ChatMessage>, Option<app::QueryLogEntry>),
    /// Progress update from background operation.
    Progress(ProgressMessage),
}

/// Progress messages from background operations.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProgressMessage {
    /// LLM request started.
    LlmStarted,
    /// LLM streaming token received.
    LlmStreaming(String),
    /// LLM request completed.
    LlmComplete(String),
    /// Database query started.
    DbStarted,
    /// Database query completed.
    DbComplete,
    /// Operation encountered an error.
    Error(String),
    /// Operation was cancelled.
    Cancelled,
}

/// The main TUI application runner.
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    event_handler: EventHandler,
    /// Flag to signal cancellation of pending operations.
    shutdown_flag: Arc<AtomicBool>,
    /// Cancellation tokens for pending requests, keyed by RequestId.
    pending_cancellations: std::collections::HashMap<RequestId, CancellationToken>,
    /// Current queue depth from orchestrator.
    queue_depth: usize,
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
            pending_cancellations: std::collections::HashMap::new(),
            queue_depth: 0,
        })
    }

    /// Cancels all pending requests.
    fn cancel_all_pending(&mut self) {
        for (_, token) in self.pending_cancellations.drain() {
            token.cancel();
        }
    }

    /// Cancels a specific pending request by ID.
    #[allow(dead_code)]
    fn cancel_request(&mut self, id: RequestId) {
        if let Some(token) = self.pending_cancellations.remove(&id) {
            token.cancel();
        }
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
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )
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
            DisableMouseCapture,
            DisableBracketedPaste
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
            let _ = execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
            );
            original_hook(panic_info);
        }));

        let mut app_state = App::new(connection);

        while app_state.running {
            // Draw the UI
            self.terminal
                .draw(|frame| ui::render(frame, &mut app_state))
                .map_err(|e| GlanceError::internal(format!("Failed to draw: {e}")))?;

            // Handle events
            if let Some(event) = self.event_handler.next()? {
                // Filter for Press events only (same as async version)
                if let Event::Key(ref key) = event {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle input submission (when palette is not open)
                    if key.code == KeyCode::Enter
                        && app_state.focus == app::Focus::Input
                        && !app_state.command_palette.visible
                    {
                        if let Some(input) = app_state.submit_input() {
                            // In limited mode, just show the input as a user message
                            app_state.add_message(app::ChatMessage::User(input.clone()));
                            app_state.add_message(app::ChatMessage::System(
                                "No database connection. Running in limited mode.".to_string(),
                            ));
                        }
                        continue;
                    }
                }

                app_state.handle_event(event);

                // Check if command palette requested immediate submission
                if app_state.command_palette.take_submit_request() {
                    if let Some(input) = app_state.submit_input() {
                        app_state.add_message(app::ChatMessage::User(input.clone()));
                        app_state.add_message(app::ChatMessage::System(
                            "No database connection. Running in limited mode.".to_string(),
                        ));
                    }
                }
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
        orchestrator: Orchestrator,
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

        // Channel for progress updates and orchestrator responses
        let (progress_tx, mut progress_rx) = mpsc::channel::<ProgressMessage>(32);
        let (response_tx, mut response_rx) = mpsc::channel::<OrchestratorResponse>(32);

        // Spawn the orchestrator actor
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx, response_tx);
        let actor_task = tokio::spawn(actor.run());

        let result = self
            .run_event_loop(
                &mut app_state,
                handle.clone(),
                &mut response_rx,
                &mut progress_rx,
            )
            .await;

        // Cleanup: signal shutdown and cancel all pending requests
        self.signal_shutdown();
        self.cancel_all_pending();

        // Close the actor gracefully
        if let Err(e) = handle.close().await {
            warn!("Error closing orchestrator actor: {}", e);
        }

        // Wait for actor to finish
        if let Err(e) = actor_task.await {
            warn!("Actor task panicked: {}", e);
        }

        // Restore panic hook
        let _ = panic::take_hook();

        result
    }

    /// The main event loop, separated for cleaner error handling.
    async fn run_event_loop(
        &mut self,
        app_state: &mut App,
        handle: OrchestratorHandle,
        response_rx: &mut mpsc::Receiver<OrchestratorResponse>,
        progress_rx: &mut mpsc::Receiver<ProgressMessage>,
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
                .draw(|frame| ui::render(frame, &mut *app_state))
                .map_err(|e| GlanceError::internal(format!("Failed to draw: {e}")))?;

            if !app_state.running || self.is_shutdown() {
                break;
            }

            // Use tokio::select! to handle events, async messages, and progress updates
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
                            handle.clone(),
                        ).await;
                    }
                }

                // Handle orchestrator responses
                Some(response) = response_rx.recv() => {
                    self.handle_orchestrator_response(response, app_state);
                }

                // Handle progress messages from the actor
                Some(progress) = progress_rx.recv() => {
                    self.handle_progress_message(progress, app_state);
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
        handle: OrchestratorHandle,
    ) {
        use crossterm::event::Event as CEvent;

        match event {
            CEvent::Key(key) if key.kind == KeyEventKind::Press => {
                // Handle cancellation during processing
                if app_state.is_processing {
                    match key.code {
                        KeyCode::Esc => {
                            // Cancel the current operation
                            let _ = handle.cancel_current().await;
                            return;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Cancel all operations (don't exit)
                            let _ = handle.cancel_all().await;
                            self.cancel_all_pending();
                            return;
                        }
                        _ => return, // Ignore other keys while processing
                    }
                }

                // Handle confirmation dialog
                if app_state.has_pending_query() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            // Confirm the query - submit to queue
                            if let Some(pending) = app_state.take_pending_query() {
                                let id = RequestId::new();
                                let token = CancellationToken::new();
                                self.pending_cancellations.insert(id, token.clone());
                                app_state.is_processing = true;
                                let _ = handle.confirm_query(id, pending.sql, token).await;
                            }
                            return;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            // Cancel the pending query - get SQL before clearing
                            let sql = app_state.pending_query.as_ref().map(|p| p.sql.clone());
                            app_state.clear_pending_query();
                            let _ = handle.cancel_pending_query(sql).await;
                            return;
                        }
                        _ => return, // Ignore other keys when dialog is shown
                    }
                }

                // Handle global shortcuts
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Copy selection if present, otherwise exit
                        if app_state.text_selection.is_some() {
                            let our_event = Event::Key(key);
                            app_state.handle_event(our_event);
                        } else {
                            app_state.running = false;
                        }
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

                // Handle input submission (but not when command palette is open)
                if key.code == KeyCode::Enter
                    && app_state.focus == app::Focus::Input
                    && !app_state.command_palette.visible
                {
                    // Close SQL completion popup if open (Enter submits, doesn't accept completion)
                    app_state.sql_completion.close();
                    if let Some(input) = app_state.submit_input() {
                        // Add user message to chat
                        app_state.add_message(app::ChatMessage::User(input.clone()));
                        app_state.is_processing = true;

                        // Submit to orchestrator queue
                        let id = RequestId::new();
                        let token = CancellationToken::new();
                        self.pending_cancellations.insert(id, token.clone());
                        let _ = handle.process_input(id, input, token).await;
                    }
                    return;
                }

                // Convert to our Event type and handle normally
                let our_event = Event::Key(key);
                app_state.handle_event(our_event);

                // Check if command palette requested immediate submission
                if app_state.command_palette.take_submit_request() {
                    if let Some(input) = app_state.submit_input() {
                        app_state.add_message(app::ChatMessage::User(input.clone()));
                        app_state.is_processing = true;

                        // Submit to orchestrator queue
                        let id = RequestId::new();
                        let token = CancellationToken::new();
                        self.pending_cancellations.insert(id, token.clone());
                        let _ = handle.process_input(id, input, token).await;
                    }
                }

                // Check if rerun was requested (from 'r' key in Normal mode)
                if let Some(sql) = app_state.take_rerun_request() {
                    let input = format!("/sql {}", sql);
                    app_state.add_message(app::ChatMessage::User(input.clone()));
                    app_state.is_processing = true;

                    // Submit to orchestrator queue
                    let id = RequestId::new();
                    let token = CancellationToken::new();
                    self.pending_cancellations.insert(id, token.clone());
                    let _ = handle.process_input(id, input, token).await;
                }
            }
            CEvent::Mouse(mouse) => {
                app_state.handle_event(Event::Mouse(mouse));
            }
            CEvent::Resize(w, h) => {
                app_state.handle_event(Event::Resize(w, h));
            }
            _ => {}
        }
    }

    /// Handles an orchestrator response.
    fn handle_orchestrator_response(
        &mut self,
        response: OrchestratorResponse,
        app_state: &mut App,
    ) {
        match response {
            OrchestratorResponse::Queued { id, position } => {
                // Request was queued - could show queue position in UI
                tracing::debug!("Request {} queued at position {}", id, position);
            }
            OrchestratorResponse::Started { id } => {
                // Request started processing
                tracing::debug!("Request {} started", id);
            }
            OrchestratorResponse::Completed { id, result } => {
                // Remove from pending cancellations
                self.pending_cancellations.remove(&id);
                app_state.is_processing = self.has_pending_requests();

                match result {
                    InputResult::Messages(messages, log_entry) => {
                        for m in messages {
                            app_state.add_message(m);
                        }
                        if let Some(entry) = log_entry {
                            app_state.add_query_log(entry);
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
                    InputResult::ConnectionSwitch {
                        messages,
                        connection_info,
                        schema,
                    } => {
                        for m in messages {
                            app_state.add_message(m);
                        }
                        app_state.connection_info = Some(connection_info);
                        app_state.is_connected = true;
                        app_state.schema = Some(schema);
                    }
                    InputResult::SchemaRefresh { messages, schema } => {
                        for m in messages {
                            app_state.add_message(m);
                        }
                        app_state.schema = Some(schema);
                    }
                    InputResult::None => {}
                }
            }
            OrchestratorResponse::QueryCompleted {
                id,
                messages,
                log_entry,
            } => {
                // Remove from pending cancellations
                self.pending_cancellations.remove(&id);
                app_state.is_processing = self.has_pending_requests();
                app_state.spinner = None;

                for m in messages {
                    app_state.add_message(m);
                }
                if let Some(entry) = log_entry {
                    app_state.add_query_log(entry);
                }
            }
            OrchestratorResponse::Failed { id, error } => {
                // Remove from pending cancellations
                self.pending_cancellations.remove(&id);
                app_state.is_processing = self.has_pending_requests();
                app_state.spinner = None;

                app_state.add_message(app::ChatMessage::Error(error));
            }
            OrchestratorResponse::Cancelled { id } => {
                // Remove from pending cancellations
                self.pending_cancellations.remove(&id);
                app_state.is_processing = self.has_pending_requests();
                app_state.spinner = None;

                app_state.add_message(app::ChatMessage::System("Operation cancelled.".to_string()));
            }
            OrchestratorResponse::NeedsConfirmation {
                id,
                sql,
                classification,
            } => {
                // Remove from pending cancellations and stop processing spinner
                // so the confirmation dialog can receive user input
                self.pending_cancellations.remove(&id);
                app_state.is_processing = self.has_pending_requests();
                app_state.spinner = None;
                // Show confirmation dialog
                app_state.set_pending_query(sql, classification);
            }
            OrchestratorResponse::QueueUpdate {
                queue_depth,
                current: _,
            } => {
                self.queue_depth = queue_depth;
                // Could update UI to show queue depth
            }
            OrchestratorResponse::QueueFull { id } => {
                // Remove from pending cancellations since it wasn't queued
                self.pending_cancellations.remove(&id);
                app_state.add_message(app::ChatMessage::Error(
                    "Queue is full. Please wait for pending requests to complete.".to_string(),
                ));
            }
            OrchestratorResponse::PendingQueryCancelled { message } => {
                app_state.add_message(message);
            }
        }
    }

    /// Returns true if there are pending requests.
    fn has_pending_requests(&self) -> bool {
        !self.pending_cancellations.is_empty()
    }

    /// Handles a progress message from the orchestrator actor.
    fn handle_progress_message(&self, progress: ProgressMessage, app_state: &mut App) {
        use crate::tui::widgets::spinner::Spinner;
        match progress {
            ProgressMessage::LlmStarted => {
                app_state.spinner = Some(Spinner::thinking());
            }
            ProgressMessage::LlmStreaming(_token) => {
                // Future: could display streaming tokens
                if app_state.spinner.is_none() {
                    app_state.spinner = Some(Spinner::thinking());
                }
            }
            ProgressMessage::LlmComplete(_) => {
                app_state.spinner = None;
            }
            ProgressMessage::DbStarted => {
                app_state.spinner = Some(Spinner::executing());
            }
            ProgressMessage::DbComplete => {
                app_state.spinner = None;
            }
            ProgressMessage::Error(msg) => {
                app_state.is_processing = false;
                app_state.spinner = None;
                app_state.add_message(app::ChatMessage::Error(msg));
            }
            ProgressMessage::Cancelled => {
                app_state.is_processing = false;
                app_state.spinner = None;
                app_state.add_message(app::ChatMessage::System("Operation cancelled.".to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cancellation_token_cancels_task() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Spawn a task that waits for cancellation
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                    "completed"
                }
                _ = token_clone.cancelled() => {
                    "cancelled"
                }
            }
        });

        // Cancel immediately
        token.cancel();

        // Task should complete with "cancelled"
        let result = handle.await.unwrap();
        assert_eq!(result, "cancelled");
    }

    #[tokio::test]
    async fn test_progress_message_cancelled_variant() {
        // Verify the Cancelled variant exists and can be matched
        let msg = ProgressMessage::Cancelled;
        match msg {
            ProgressMessage::Cancelled => {}
            _ => panic!("Expected Cancelled variant"),
        }
    }

    #[test]
    fn test_async_message_progress_variant() {
        // Verify AsyncMessage can carry ProgressMessage::Cancelled
        let msg = AsyncMessage::Progress(ProgressMessage::Cancelled);
        match msg {
            AsyncMessage::Progress(ProgressMessage::Cancelled) => {}
            _ => panic!("Expected Progress(Cancelled) variant"),
        }
    }
}
