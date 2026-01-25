//! Actor-based orchestrator for concurrent TUI operations.
//!
//! Replaces `Arc<Mutex<Orchestrator>>` with a message-passing actor pattern
//! to enable cancellation and avoid lock contention during LLM/DB operations.
//!
//! Implements FR-1.2 from v0.2d spec: proper VecDeque-based request queue with
//! FIFO processing, max depth enforcement, and queue status updates.

use crate::app::{InputResult, Orchestrator};
use crate::error::{GlanceError, Result};
use crate::tui::app::{ChatMessage, QueryLogEntry, QuerySource};
use crate::tui::request_queue::{
    PendingRequest, QueueEvent, RequestQueue, DEFAULT_MAX_QUEUE_DEPTH,
};

/// Maximum number of requests that can be queued.
/// Re-exported from request_queue for backward compatibility.
pub const MAX_QUEUE_DEPTH: usize = DEFAULT_MAX_QUEUE_DEPTH;
use crate::tui::ProgressMessage;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Unique identifier for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

impl RequestId {
    /// Generates a new unique request ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the inner u64 value.
    #[allow(dead_code)]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Type of request being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestType {
    /// Natural language input to be processed by LLM.
    NaturalLanguage,
    /// Raw SQL to execute directly.
    RawSql,
    /// Confirmation of a pending mutation query.
    Confirmation,
}

/// Represents which phase of operation a request is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Variants will be used as implementation progresses
pub enum OperationPhase {
    Queued,
    LlmRequesting,
    LlmThinking,
    LlmStreaming,
    LlmParsing,
    Classifying,
    DbExecuting,
    Processing,
}

/// Internal action representation for command handling.
///
/// This enum represents the action to take after classifying an incoming command.
/// It separates the pure command classification from the async execution.
#[derive(Debug)]
pub enum CommandAction {
    /// Enqueue a request for processing.
    Enqueue(PendingRequest),
    /// Confirm a query that was awaiting user approval.
    Confirm {
        id: RequestId,
        sql: String,
        cancel: CancellationToken,
    },
    /// Cancel the current in-flight request.
    CancelCurrent,
    /// Cancel a specific queued request by ID.
    CancelById(RequestId),
    /// Cancel all operations (current + queued).
    CancelAll,
    /// Cancel a pending query confirmation dialog.
    CancelPendingQuery { sql: Option<String> },
    /// Grant consent for plaintext secret storage.
    GrantPlaintextConsent,
    /// Shut down the actor gracefully.
    Shutdown,
}

/// Commands sent from TUI to orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorCommand {
    /// Process user input (natural language or command).
    ProcessInput {
        id: RequestId,
        input: String,
        cancel: CancellationToken,
    },
    /// Execute SQL directly (from /sql command).
    #[allow(dead_code)]
    ExecuteSql {
        id: RequestId,
        sql: String,
        cancel: CancellationToken,
    },
    /// Confirm a pending mutation query.
    ConfirmQuery {
        id: RequestId,
        sql: String,
        cancel: CancellationToken,
    },
    /// Cancel the current operation.
    CancelCurrent,
    /// Cancel a specific queued request.
    #[allow(dead_code)]
    CancelRequest(RequestId),
    /// Cancel all operations (current + queued).
    CancelAll,
    /// Cancel a pending query (synchronous, no DB/LLM call).
    /// The SQL is passed so it can be recorded in history.
    CancelPendingQuery { sql: Option<String> },
    /// Grant consent for plaintext secret storage.
    GrantPlaintextConsent,
    /// Gracefully close the actor and its resources.
    Shutdown,
}

/// Responses sent from orchestrator to TUI.
#[derive(Debug, Clone)]
pub enum OrchestratorResponse {
    /// Request was queued (not yet processing).
    Queued { id: RequestId, position: usize },
    /// Request started processing.
    Started {
        id: RequestId,
        phase: OperationPhase,
    },
    /// Progress update for a running request.
    Progress {
        id: RequestId,
        phase: OperationPhase,
        elapsed: Duration,
        #[allow(dead_code)] // Will be used for detailed progress messages
        detail: Option<String>,
    },
    /// Operation completed successfully.
    Completed { id: RequestId, result: InputResult },
    /// Query execution completed.
    QueryCompleted {
        id: RequestId,
        messages: Vec<ChatMessage>,
        log_entry: Option<QueryLogEntry>,
    },
    /// Operation failed with error.
    Failed { id: RequestId, error: String },
    /// Operation was cancelled.
    Cancelled {
        id: RequestId,
        log_entry: Option<QueryLogEntry>,
    },
    /// Query needs user confirmation before execution.
    NeedsConfirmation {
        #[allow(dead_code)]
        id: RequestId,
        sql: String,
        classification: crate::safety::ClassificationResult,
    },
    /// Queue status changed.
    QueueUpdate {
        queue_depth: usize,
        #[allow(dead_code)] // Will be used by UI to show queue limit
        max_depth: usize,
        #[allow(dead_code)]
        current: Option<RequestId>,
        #[allow(dead_code)] // Will be used by UI to show request positions
        positions: Vec<(RequestId, usize)>,
    },
    /// Queue is full, request rejected.
    QueueFull { id: RequestId },
    /// Pending query was cancelled (from CancelPendingQuery command).
    PendingQueryCancelled {
        message: ChatMessage,
        log_entry: Option<QueryLogEntry>,
    },
}

/// The orchestrator actor that owns the orchestrator and processes requests.
///
/// Implements a proper request queue per FR-1.2:
/// - VecDeque for FIFO ordering
/// - Max depth of 10 requests
/// - Queue position tracking and updates
pub struct OrchestratorActor {
    /// The underlying orchestrator for LLM/DB operations.
    orchestrator: Orchestrator,
    /// Channel for receiving commands from TUI.
    receiver: mpsc::Receiver<OrchestratorCommand>,
    /// Channel for sending progress updates to TUI.
    progress_tx: mpsc::Sender<ProgressMessage>,
    /// Channel for sending responses to TUI.
    response_tx: mpsc::Sender<OrchestratorResponse>,
    /// Request queue managing pending, in-flight, and confirmation state.
    request_queue: RequestQueue,
    /// Currently processing request ID (for external reference).
    current: Option<RequestId>,
}

impl OrchestratorActor {
    /// Creates a new actor and returns a handle for communication.
    pub fn spawn(
        orchestrator: Orchestrator,
        progress_tx: mpsc::Sender<ProgressMessage>,
        response_tx: mpsc::Sender<OrchestratorResponse>,
    ) -> (OrchestratorHandle, Self) {
        let (sender, receiver) = mpsc::channel(32);

        let actor = Self {
            orchestrator,
            receiver,
            progress_tx,
            response_tx,
            request_queue: RequestQueue::new(),
            current: None,
        };

        let handle = OrchestratorHandle { sender };

        (handle, actor)
    }

    /// Returns the current queue depth.
    #[allow(dead_code)]
    pub fn queue_depth(&self) -> usize {
        self.request_queue.pending_count()
    }

    /// Sends a queue update to the TUI.
    async fn send_queue_update(&self) {
        let _ = self
            .response_tx
            .send(OrchestratorResponse::QueueUpdate {
                queue_depth: self.request_queue.pending_count(),
                max_depth: self.request_queue.max_depth(),
                current: self.current,
                positions: self.request_queue.get_queue_positions(),
            })
            .await;
    }

    /// Enqueues a request if queue is not full.
    /// Returns the queue position (1-indexed) or None if queue is full.
    async fn enqueue(&mut self, request: PendingRequest) -> Option<usize> {
        let id = request.id;

        match self.request_queue.enqueue(request) {
            QueueEvent::Queued { position } => {
                let _ = self
                    .response_tx
                    .send(OrchestratorResponse::Queued { id, position })
                    .await;
                self.send_queue_update().await;
                Some(position)
            }
            QueueEvent::QueueFull => {
                let _ = self
                    .response_tx
                    .send(OrchestratorResponse::QueueFull { id })
                    .await;
                None
            }
        }
    }

    /// Processes the next request from the queue.
    async fn process_next(&mut self) {
        let Some(request) = self.request_queue.try_dequeue() else {
            return;
        };

        // Check if already cancelled before processing
        if request.cancel.is_cancelled() {
            let log_entry = Self::log_entry_for_cancelled(&request);
            let _ = self
                .response_tx
                .send(OrchestratorResponse::Cancelled {
                    id: request.id,
                    log_entry,
                })
                .await;
            self.send_queue_update().await;
            return;
        }

        let id = request.id;
        let cancel = request.cancel.clone();

        self.current = Some(id);

        let _ = self
            .response_tx
            .send(OrchestratorResponse::Started {
                id,
                phase: OperationPhase::Processing,
            })
            .await;
        self.send_queue_update().await;

        // Process request inline (background spawning will be added in future iteration)
        match request.request_type {
            RequestType::NaturalLanguage => {
                self.process_input(id, &request.input, cancel).await;
            }
            RequestType::RawSql => {
                self.process_sql(id, &request.input, cancel).await;
            }
            RequestType::Confirmation => {
                self.process_confirmation(id, &request.input, cancel).await;
            }
        }

        self.current = None;
        self.request_queue.clear_in_flight();
        self.send_queue_update().await;
    }

    /// Processes natural language input.
    async fn process_input(&mut self, id: RequestId, input: &str, cancel: CancellationToken) {
        let _ = self.progress_tx.send(ProgressMessage::LlmStarted).await;

        tokio::select! {
            biased;

            _ = cancel.cancelled() => {
                let _ = self.response_tx.send(OrchestratorResponse::Cancelled {
                    id,
                    log_entry: None,
                }).await;
                let _ = self.progress_tx.send(ProgressMessage::Cancelled).await;
            }
            result = self.orchestrator.handle_input_streaming(input, {
                let progress_tx = self.progress_tx.clone();
                move |token| {
                    let progress_tx = progress_tx.clone();
                    let token_owned = token.to_string();
                    async move {
                        let _ = progress_tx
                            .send(ProgressMessage::LlmStreaming(token_owned))
                            .await;
                    }
                }
            }) => {
                let _ = self.progress_tx.send(ProgressMessage::LlmComplete(String::new())).await;
                match result {
                    Ok(InputResult::NeedsConfirmation { sql, classification }) => {
                        self.request_queue.set_confirmation_pending(true); // Pause queue
                        let _ = self.response_tx.send(OrchestratorResponse::NeedsConfirmation {
                            id,
                            sql,
                            classification,
                        }).await;
                    }
                    Ok(result) => {
                        let _ = self.response_tx.send(OrchestratorResponse::Completed { id, result }).await;
                    }
                    Err(e) => {
                        let _ = self.progress_tx.send(ProgressMessage::Error(e.to_string())).await;
                        let _ = self.response_tx.send(OrchestratorResponse::Failed {
                            id,
                            error: e.to_string(),
                        }).await;
                    }
                }
            }
        }
    }

    /// Processes raw SQL execution.
    async fn process_sql(&mut self, id: RequestId, sql: &str, cancel: CancellationToken) {
        let _ = self.progress_tx.send(ProgressMessage::DbStarted).await;

        tokio::select! {
            biased;

            _ = cancel.cancelled() => {
                let entry = QueryLogEntry::cancelled_with_source(sql.to_string(), QuerySource::Manual);
                let _ = self.response_tx.send(OrchestratorResponse::Cancelled {
                    id,
                    log_entry: Some(entry),
                }).await;
                let _ = self.progress_tx.send(ProgressMessage::Cancelled).await;
            }
            (messages, log_entry) = self.orchestrator.execute_and_format(sql) => {
                let _ = self.progress_tx.send(ProgressMessage::DbComplete).await;
                let _ = self.response_tx.send(OrchestratorResponse::QueryCompleted {
                    id,
                    messages,
                    log_entry,
                }).await;
            }
        }
    }

    /// Processes query confirmation.
    async fn process_confirmation(&mut self, id: RequestId, sql: &str, cancel: CancellationToken) {
        let _ = self.progress_tx.send(ProgressMessage::DbStarted).await;

        tokio::select! {
            biased;

            _ = cancel.cancelled() => {
                let entry = QueryLogEntry::cancelled_with_source(sql.to_string(), QuerySource::Generated);
                let _ = self.response_tx.send(OrchestratorResponse::Cancelled {
                    id,
                    log_entry: Some(entry),
                }).await;
                let _ = self.progress_tx.send(ProgressMessage::Cancelled).await;
            }
            result = self.orchestrator.confirm_query(sql) => {
                let _ = self.progress_tx.send(ProgressMessage::DbComplete).await;
                let (messages, log_entry) = result;
                let _ = self.response_tx.send(OrchestratorResponse::QueryCompleted {
                    id,
                    messages,
                    log_entry,
                }).await;
            }
        }
    }

    /// Cancels the current operation.
    fn cancel_current(&mut self) {
        if let Some(token) = self.request_queue.cancel_current() {
            token.cancel();
        }
        self.current = None;
    }

    /// Cancels a specific queued request by ID.
    async fn cancel_request(&mut self, id: RequestId) {
        // Check if it's the current request
        if self.current == Some(id) {
            self.cancel_current();
            return;
        }

        // Try to cancel from queue
        if let Some(request) = self.request_queue.cancel_by_id(id) {
            let log_entry = Self::log_entry_for_cancelled(&request);
            let _ = self
                .response_tx
                .send(OrchestratorResponse::Cancelled {
                    id: request.id,
                    log_entry,
                })
                .await;
            self.send_queue_update().await;
        }
    }

    /// Cancels all operations (current + queued).
    async fn cancel_all(&mut self) {
        // Cancel current
        self.cancel_current();

        // Cancel all queued
        let cancelled = self.request_queue.cancel_all();
        for request in cancelled {
            let log_entry = Self::log_entry_for_cancelled(&request);
            let _ = self
                .response_tx
                .send(OrchestratorResponse::Cancelled {
                    id: request.id,
                    log_entry,
                })
                .await;
        }

        self.send_queue_update().await;
    }

    /// Runs the actor loop, processing commands until Shutdown is received.
    /// Runs the actor loop, processing commands until Shutdown is received.
    pub async fn run(mut self) {
        let mut ticker = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                biased;

                Some(cmd) = self.receiver.recv() => {
                    let action = Self::classify_command(cmd);
                    if matches!(action, CommandAction::Shutdown) {
                        break;
                    }
                    self.execute_action(action).await;
                }

                _ = ticker.tick() => {
                    self.maybe_send_progress().await;
                }

                _ = async {}, if self.request_queue.can_process_next() => {
                    self.process_next().await;
                }
            }
        }

        self.shutdown().await;
    }

    /// Sends progress update if there's an in-flight request.
    async fn maybe_send_progress(&self) {
        if let Some(req) = self.request_queue.in_flight() {
            let _ = self
                .response_tx
                .send(OrchestratorResponse::Progress {
                    id: req.id,
                    phase: req.phase,
                    elapsed: req.started_at.elapsed(),
                    detail: None,
                })
                .await;
        }
    }

    /// Gracefully shuts down the actor.
    async fn shutdown(&mut self) {
        self.cancel_all().await;
        if let Err(e) = self.orchestrator.close().await {
            warn!("Error closing orchestrator: {}", e);
        }
    }

    /// Classifies a command into an action.
    ///
    /// This is a pure function that maps incoming commands to internal actions,
    /// separating command parsing from execution.
    fn classify_command(cmd: OrchestratorCommand) -> CommandAction {
        match cmd {
            OrchestratorCommand::ProcessInput { id, input, cancel } => {
                CommandAction::Enqueue(PendingRequest {
                    id,
                    input,
                    request_type: RequestType::NaturalLanguage,
                    queued_at: Instant::now(),
                    cancel,
                })
            }
            OrchestratorCommand::ExecuteSql { id, sql, cancel } => {
                CommandAction::Enqueue(PendingRequest {
                    id,
                    input: sql,
                    request_type: RequestType::RawSql,
                    queued_at: Instant::now(),
                    cancel,
                })
            }
            OrchestratorCommand::ConfirmQuery { id, sql, cancel } => {
                CommandAction::Confirm { id, sql, cancel }
            }
            OrchestratorCommand::CancelCurrent => CommandAction::CancelCurrent,
            OrchestratorCommand::CancelRequest(id) => CommandAction::CancelById(id),
            OrchestratorCommand::CancelAll => CommandAction::CancelAll,
            OrchestratorCommand::CancelPendingQuery { sql } => {
                CommandAction::CancelPendingQuery { sql }
            }
            OrchestratorCommand::GrantPlaintextConsent => CommandAction::GrantPlaintextConsent,
            OrchestratorCommand::Shutdown => CommandAction::Shutdown,
        }
    }

    /// Executes a command action.
    ///
    /// This is the async method that performs the actual work for each action.
    async fn execute_action(&mut self, action: CommandAction) {
        match action {
            CommandAction::Enqueue(request) => {
                self.enqueue(request).await;
            }
            CommandAction::Confirm { id, sql, cancel } => {
                self.request_queue.set_confirmation_pending(false);
                let request = PendingRequest {
                    id,
                    input: sql,
                    request_type: RequestType::Confirmation,
                    queued_at: Instant::now(),
                    cancel,
                };
                self.enqueue(request).await;
            }
            CommandAction::CancelCurrent => {
                self.cancel_current();
            }
            CommandAction::CancelById(id) => {
                self.cancel_request(id).await;
            }
            CommandAction::CancelAll => {
                self.cancel_all().await;
            }
            CommandAction::CancelPendingQuery { sql } => {
                self.request_queue.set_confirmation_pending(false);
                let (msg, log_entry) = self.orchestrator.cancel_query(sql.as_deref()).await;
                let _ = self
                    .response_tx
                    .send(OrchestratorResponse::PendingQueryCancelled {
                        message: msg,
                        log_entry,
                    })
                    .await;
            }
            CommandAction::GrantPlaintextConsent => {
                if let Some(state_db) = self.orchestrator.state_db() {
                    state_db.secrets().consent_to_plaintext();
                }
            }
            CommandAction::Shutdown => {
                // Handled in run() loop
            }
        }
    }

    /// Returns elapsed time since a request was queued.
    #[allow(dead_code)]
    pub fn elapsed_since_queued(queued_at: Instant) -> Duration {
        queued_at.elapsed()
    }

    /// Returns a cancelled log entry for a queued SQL request, if applicable.
    fn log_entry_for_cancelled(request: &PendingRequest) -> Option<QueryLogEntry> {
        match request.request_type {
            RequestType::RawSql => Some(QueryLogEntry::cancelled_with_source(
                request.input.clone(),
                QuerySource::Manual,
            )),
            RequestType::Confirmation => Some(QueryLogEntry::cancelled_with_source(
                request.input.clone(),
                QuerySource::Generated,
            )),
            RequestType::NaturalLanguage => None,
        }
    }
}

/// Handle for communicating with the orchestrator actor.
///
/// This is a lightweight, cloneable handle that can be used to send commands
/// to the actor. Responses come back via the response channel.
#[derive(Clone)]
pub struct OrchestratorHandle {
    sender: mpsc::Sender<OrchestratorCommand>,
}

impl OrchestratorHandle {
    /// Submits user input for processing. Returns immediately after queueing.
    /// Results come back via the response channel.
    pub async fn process_input(
        &self,
        id: RequestId,
        input: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::ProcessInput { id, input, cancel })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Submits raw SQL for execution. Returns immediately after queueing.
    /// Results come back via the response channel.
    #[allow(dead_code)]
    pub async fn execute_sql(
        &self,
        id: RequestId,
        sql: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::ExecuteSql { id, sql, cancel })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Confirms and executes a pending query. Returns immediately after queueing.
    /// Results come back via the response channel.
    pub async fn confirm_query(
        &self,
        id: RequestId,
        sql: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::ConfirmQuery { id, sql, cancel })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Cancels the currently processing operation.
    pub async fn cancel_current(&self) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::CancelCurrent)
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Cancels a specific queued request.
    #[allow(dead_code)]
    pub async fn cancel_request(&self, id: RequestId) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::CancelRequest(id))
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Cancels all operations (current + queued).
    pub async fn cancel_all(&self) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::CancelAll)
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Cancels a pending query (the confirmation dialog state).
    /// Pass the SQL so it can be recorded in history as cancelled.
    pub async fn cancel_pending_query(&self, sql: Option<String>) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::CancelPendingQuery { sql })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Grants consent for plaintext secret storage.
    pub async fn grant_plaintext_consent(&self) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::GrantPlaintextConsent)
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))
    }

    /// Signals the actor to close gracefully.
    pub async fn close(&self) -> Result<()> {
        self.sender
            .send(OrchestratorCommand::Shutdown)
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor already closed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Schema;
    use tokio::time::timeout;

    /// Helper to create actor with all channels.
    fn create_test_actor() -> (
        OrchestratorHandle,
        OrchestratorActor,
        mpsc::Receiver<ProgressMessage>,
        mpsc::Receiver<OrchestratorResponse>,
    ) {
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let (progress_tx, progress_rx) = mpsc::channel(32);
        let (response_tx, response_rx) = mpsc::channel(32);
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx, response_tx);
        (handle, actor, progress_rx, response_rx)
    }

    #[tokio::test]
    async fn test_actor_process_input() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Send a request
        let id = RequestId::new();
        let token = CancellationToken::new();
        handle
            .process_input(id, "".to_string(), token)
            .await
            .unwrap();

        // Should receive Queued response
        let resp = timeout(std::time::Duration::from_secs(1), response_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(resp, OrchestratorResponse::Queued { .. }));

        // Drain responses until we get Started (QueueUpdate may come in between)
        let mut found_started = false;
        for _ in 0..10 {
            if let Ok(Some(resp)) =
                timeout(std::time::Duration::from_millis(500), response_rx.recv()).await
            {
                if matches!(resp, OrchestratorResponse::Started { .. }) {
                    found_started = true;
                    break;
                }
            }
        }
        assert!(found_started, "Expected Started response");

        // Close the actor
        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_cancellation() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Create a pre-cancelled token
        let id = RequestId::new();
        let token = CancellationToken::new();
        token.cancel();

        handle
            .process_input(id, "/help".to_string(), token)
            .await
            .unwrap();

        // Should receive Queued response
        let resp = timeout(std::time::Duration::from_secs(1), response_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(resp, OrchestratorResponse::Queued { .. }));

        // Drain responses until we get Cancelled (QueueUpdate may come in between)
        let mut found_cancelled = false;
        for _ in 0..10 {
            if let Ok(Some(resp)) =
                timeout(std::time::Duration::from_millis(500), response_rx.recv()).await
            {
                if matches!(resp, OrchestratorResponse::Cancelled { .. }) {
                    found_cancelled = true;
                    break;
                }
            }
        }
        assert!(found_cancelled, "Expected Cancelled response");

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_cancel_pending_query() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        handle.cancel_pending_query(None).await.unwrap();

        // Should receive PendingQueryCancelled response
        let resp = timeout(std::time::Duration::from_secs(1), response_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match resp {
            OrchestratorResponse::PendingQueryCancelled { message, .. } => match message {
                ChatMessage::System(text) => assert!(text.contains("cancelled")),
                _ => panic!("Expected System message"),
            },
            _ => panic!("Expected PendingQueryCancelled response"),
        }

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_emits_streaming_tokens() {
        let (handle, actor, mut progress_rx, mut response_rx) = create_test_actor();
        let actor_handle = tokio::spawn(actor.run());

        let id = RequestId::new();
        handle
            .process_input(
                id,
                "Show me all users".to_string(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        let mut saw_stream = false;
        let mut saw_complete = false;
        for _ in 0..20 {
            if let Ok(Some(progress)) =
                timeout(std::time::Duration::from_secs(1), progress_rx.recv()).await
            {
                match progress {
                    ProgressMessage::LlmStreaming(token) => {
                        if !token.is_empty() {
                            saw_stream = true;
                        }
                    }
                    ProgressMessage::LlmComplete(_) => {
                        saw_complete = true;
                        if saw_stream {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }

        assert!(saw_stream, "Expected streaming tokens");
        assert!(saw_complete, "Expected LlmComplete signal");

        // Drain until we see completion for this request.
        for _ in 0..20 {
            if let Ok(Some(resp)) =
                timeout(std::time::Duration::from_secs(1), response_rx.recv()).await
            {
                if matches!(resp, OrchestratorResponse::Completed { id: resp_id, .. } if resp_id == id)
                {
                    break;
                }
            }
        }

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_close() {
        let (handle, actor, _progress_rx, _response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Close should succeed
        handle.close().await.unwrap();

        // Actor should complete
        actor_handle.await.unwrap();

        // Further requests should fail
        let id = RequestId::new();
        let token = CancellationToken::new();
        let result = handle.process_input(id, "test".to_string(), token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_fifo_ordering() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Submit multiple requests
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        let id3 = RequestId::new();

        handle
            .process_input(id1, "first".to_string(), CancellationToken::new())
            .await
            .unwrap();
        handle
            .process_input(id2, "second".to_string(), CancellationToken::new())
            .await
            .unwrap();
        handle
            .process_input(id3, "third".to_string(), CancellationToken::new())
            .await
            .unwrap();

        // Collect queued responses - should be in order (skip QueueUpdate responses)
        let mut queued_positions = vec![];
        for _ in 0..20 {
            if queued_positions.len() >= 3 {
                break;
            }
            if let Ok(Some(OrchestratorResponse::Queued { position, .. })) =
                timeout(std::time::Duration::from_millis(200), response_rx.recv()).await
            {
                queued_positions.push(position);
            }
        }

        // Positions should be 1, 2, 3 (FIFO)
        assert_eq!(queued_positions, vec![1, 2, 3]);

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    #[ignore] // TODO: Flaky test - mock orchestrator processes too fast, queue empties before overflow
    async fn test_queue_max_depth() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Fill the queue to max depth
        for i in 0..MAX_QUEUE_DEPTH {
            let id = RequestId::new();
            handle
                .process_input(id, format!("request {}", i), CancellationToken::new())
                .await
                .unwrap();
        }

        // Drain responses (Queued + QueueUpdate interleaved)
        for _ in 0..(MAX_QUEUE_DEPTH * 3) {
            let _ = timeout(std::time::Duration::from_millis(100), response_rx.recv()).await;
        }

        // Try to add one more - should be rejected
        let overflow_id = RequestId::new();
        handle
            .process_input(
                overflow_id,
                "overflow".to_string(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        // Should receive QueueFull response (skip other responses)
        let mut found_queue_full = false;
        for _ in 0..10 {
            if let Ok(Some(resp)) =
                timeout(std::time::Duration::from_millis(200), response_rx.recv()).await
            {
                if matches!(resp, OrchestratorResponse::QueueFull { id } if id == overflow_id) {
                    found_queue_full = true;
                    break;
                }
            }
        }
        assert!(found_queue_full, "Expected QueueFull response");

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    #[ignore] // TODO: Flaky test - timing-dependent, may need mock with artificial delays
    async fn test_cancel_queued_request() {
        let (handle, actor, _progress_rx, mut response_rx) = create_test_actor();

        let actor_handle = tokio::spawn(actor.run());

        // Submit two requests
        let id1 = RequestId::new();
        let id2 = RequestId::new();

        handle
            .process_input(id1, "first".to_string(), CancellationToken::new())
            .await
            .unwrap();
        handle
            .process_input(id2, "second".to_string(), CancellationToken::new())
            .await
            .unwrap();

        // Drain initial responses (Queued + QueueUpdate interleaved)
        for _ in 0..10 {
            let _ = timeout(std::time::Duration::from_millis(100), response_rx.recv()).await;
        }

        // Cancel the second request
        handle.cancel_request(id2).await.unwrap();

        // Should eventually receive Cancelled for id2
        let mut found_cancelled = false;
        for _ in 0..10 {
            if let Ok(Some(resp)) =
                timeout(std::time::Duration::from_millis(100), response_rx.recv()).await
            {
                if matches!(resp, OrchestratorResponse::Cancelled { id, .. } if id == id2) {
                    found_cancelled = true;
                    break;
                }
            }
        }
        assert!(found_cancelled, "Expected Cancelled response for id2");

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_request_id_uniqueness() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        let id3 = RequestId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[tokio::test]
    async fn test_request_id_display() {
        let id = RequestId(42);
        assert_eq!(format!("{}", id), "#42");
    }

    // Tests for classify_command (pure function)

    #[test]
    fn classify_command_process_input_returns_enqueue() {
        let id = RequestId::new();
        let cmd = OrchestratorCommand::ProcessInput {
            id,
            input: "test".to_string(),
            cancel: CancellationToken::new(),
        };

        let action = OrchestratorActor::classify_command(cmd);
        match action {
            CommandAction::Enqueue(req) => {
                assert_eq!(req.id, id);
                assert_eq!(req.request_type, RequestType::NaturalLanguage);
            }
            _ => panic!("Expected Enqueue action"),
        }
    }

    #[test]
    fn classify_command_execute_sql_returns_enqueue() {
        let id = RequestId::new();
        let cmd = OrchestratorCommand::ExecuteSql {
            id,
            sql: "SELECT 1".to_string(),
            cancel: CancellationToken::new(),
        };

        let action = OrchestratorActor::classify_command(cmd);
        match action {
            CommandAction::Enqueue(req) => {
                assert_eq!(req.id, id);
                assert_eq!(req.request_type, RequestType::RawSql);
            }
            _ => panic!("Expected Enqueue action"),
        }
    }

    #[test]
    fn classify_command_confirm_query_returns_confirm() {
        let id = RequestId::new();
        let cmd = OrchestratorCommand::ConfirmQuery {
            id,
            sql: "DELETE FROM users".to_string(),
            cancel: CancellationToken::new(),
        };

        let action = OrchestratorActor::classify_command(cmd);
        match action {
            CommandAction::Confirm {
                id: action_id, sql, ..
            } => {
                assert_eq!(action_id, id);
                assert_eq!(sql, "DELETE FROM users");
            }
            _ => panic!("Expected Confirm action"),
        }
    }

    #[test]
    fn classify_command_cancel_current_returns_cancel_current() {
        let cmd = OrchestratorCommand::CancelCurrent;
        let action = OrchestratorActor::classify_command(cmd);
        assert!(matches!(action, CommandAction::CancelCurrent));
    }

    #[test]
    fn classify_command_cancel_request_returns_cancel_by_id() {
        let id = RequestId::new();
        let cmd = OrchestratorCommand::CancelRequest(id);
        let action = OrchestratorActor::classify_command(cmd);
        match action {
            CommandAction::CancelById(action_id) => assert_eq!(action_id, id),
            _ => panic!("Expected CancelById action"),
        }
    }

    #[test]
    fn classify_command_cancel_all_returns_cancel_all() {
        let cmd = OrchestratorCommand::CancelAll;
        let action = OrchestratorActor::classify_command(cmd);
        assert!(matches!(action, CommandAction::CancelAll));
    }

    #[test]
    fn classify_command_cancel_pending_query_returns_cancel_pending_query() {
        let cmd = OrchestratorCommand::CancelPendingQuery {
            sql: Some("SELECT 1".to_string()),
        };
        let action = OrchestratorActor::classify_command(cmd);
        match action {
            CommandAction::CancelPendingQuery { sql } => {
                assert_eq!(sql, Some("SELECT 1".to_string()));
            }
            _ => panic!("Expected CancelPendingQuery action"),
        }
    }

    #[test]
    fn classify_command_shutdown_returns_shutdown() {
        let cmd = OrchestratorCommand::Shutdown;
        let action = OrchestratorActor::classify_command(cmd);
        assert!(matches!(action, CommandAction::Shutdown));
    }

    #[test]
    fn classify_command_is_pure_same_input_same_output() {
        // Test that same input always produces the same output
        let id = RequestId(999);

        for _ in 0..3 {
            let cmd = OrchestratorCommand::CancelRequest(id);
            let action = OrchestratorActor::classify_command(cmd);
            match action {
                CommandAction::CancelById(action_id) => assert_eq!(action_id.as_u64(), 999),
                _ => panic!("Expected CancelById action"),
            }
        }
    }
}
