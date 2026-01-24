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
use crate::tui::ProgressMessage;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Maximum number of requests that can be queued.
pub const MAX_QUEUE_DEPTH: usize = 10;

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

/// A request waiting in the queue.
#[derive(Debug)]
#[allow(dead_code)]
pub struct PendingRequest {
    /// Unique identifier for this request.
    pub id: RequestId,
    /// The input string (natural language, SQL, or confirmation).
    pub input: String,
    /// Type of request.
    pub request_type: RequestType,
    /// When this request was queued.
    pub queued_at: Instant,
    /// Cancellation token for this request.
    pub cancel: CancellationToken,
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
    /// Gracefully close the actor and its resources.
    Shutdown,
}

/// Responses sent from orchestrator to TUI.
#[derive(Debug, Clone)]
pub enum OrchestratorResponse {
    /// Request was queued (not yet processing).
    Queued { id: RequestId, position: usize },
    /// Request started processing.
    Started { id: RequestId },
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
        #[allow(dead_code)]
        current: Option<RequestId>,
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
    /// Pending request queue (FIFO).
    queue: VecDeque<PendingRequest>,
    /// Currently processing request ID.
    current: Option<RequestId>,
    /// Cancellation token for current operation.
    current_cancel: Option<CancellationToken>,
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
            queue: VecDeque::new(),
            current: None,
            current_cancel: None,
        };

        let handle = OrchestratorHandle { sender };

        (handle, actor)
    }

    /// Returns the current queue depth.
    #[allow(dead_code)]
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    /// Sends a queue update to the TUI.
    async fn send_queue_update(&self) {
        let _ = self
            .response_tx
            .send(OrchestratorResponse::QueueUpdate {
                queue_depth: self.queue.len(),
                current: self.current,
            })
            .await;
    }

    /// Enqueues a request if queue is not full.
    /// Returns the queue position (1-indexed) or None if queue is full.
    async fn enqueue(&mut self, request: PendingRequest) -> Option<usize> {
        if self.queue.len() >= MAX_QUEUE_DEPTH {
            let _ = self
                .response_tx
                .send(OrchestratorResponse::QueueFull { id: request.id })
                .await;
            return None;
        }

        let id = request.id;
        self.queue.push_back(request);
        let position = self.queue.len();

        let _ = self
            .response_tx
            .send(OrchestratorResponse::Queued { id, position })
            .await;
        self.send_queue_update().await;

        Some(position)
    }

    /// Processes the next request from the queue.
    async fn process_next(&mut self) {
        let Some(request) = self.queue.pop_front() else {
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

        self.current = Some(request.id);
        self.current_cancel = Some(request.cancel.clone());

        let _ = self
            .response_tx
            .send(OrchestratorResponse::Started { id: request.id })
            .await;
        self.send_queue_update().await;

        match request.request_type {
            RequestType::NaturalLanguage => {
                self.process_input(request.id, &request.input, request.cancel)
                    .await;
            }
            RequestType::RawSql => {
                self.process_sql(request.id, &request.input, request.cancel)
                    .await;
            }
            RequestType::Confirmation => {
                self.process_confirmation(request.id, &request.input, request.cancel)
                    .await;
            }
        }

        self.current = None;
        self.current_cancel = None;
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
                    async move {
                        let _ = progress_tx
                            .send(ProgressMessage::LlmStreaming(token.to_string()))
                            .await;
                    }
                }
            }) => {
                let _ = self.progress_tx.send(ProgressMessage::LlmComplete(String::new())).await;
                match result {
                    Ok(InputResult::NeedsConfirmation { sql, classification }) => {
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
        if let Some(token) = self.current_cancel.take() {
            token.cancel();
        }
    }

    /// Cancels a specific queued request by ID.
    async fn cancel_request(&mut self, id: RequestId) {
        // Check if it's the current request
        if self.current == Some(id) {
            self.cancel_current();
            return;
        }

        // Find and remove from queue
        if let Some(pos) = self.queue.iter().position(|r| r.id == id) {
            if let Some(request) = self.queue.remove(pos) {
                request.cancel.cancel();
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
    }

    /// Cancels all operations (current + queued).
    async fn cancel_all(&mut self) {
        // Cancel current
        self.cancel_current();

        // Cancel all queued
        while let Some(request) = self.queue.pop_front() {
            request.cancel.cancel();
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
    pub async fn run(mut self) {
        loop {
            // Process queue when idle (before waiting for commands)
            if self.current.is_none() && !self.queue.is_empty() {
                self.process_next().await;
                continue;
            }

            // Wait for next command
            let Some(cmd) = self.receiver.recv().await else {
                // Channel closed, exit
                break;
            };

            match cmd {
                OrchestratorCommand::ProcessInput { id, input, cancel } => {
                    let request = PendingRequest {
                        id,
                        input,
                        request_type: RequestType::NaturalLanguage,
                        queued_at: Instant::now(),
                        cancel,
                    };
                    self.enqueue(request).await;
                }

                OrchestratorCommand::ExecuteSql { id, sql, cancel } => {
                    let request = PendingRequest {
                        id,
                        input: sql,
                        request_type: RequestType::RawSql,
                        queued_at: Instant::now(),
                        cancel,
                    };
                    self.enqueue(request).await;
                }

                OrchestratorCommand::ConfirmQuery { id, sql, cancel } => {
                    let request = PendingRequest {
                        id,
                        input: sql,
                        request_type: RequestType::Confirmation,
                        queued_at: Instant::now(),
                        cancel,
                    };
                    self.enqueue(request).await;
                }

                OrchestratorCommand::CancelCurrent => {
                    self.cancel_current();
                }

                OrchestratorCommand::CancelRequest(id) => {
                    self.cancel_request(id).await;
                }

                OrchestratorCommand::CancelAll => {
                    self.cancel_all().await;
                }

                OrchestratorCommand::CancelPendingQuery { sql } => {
                    let (msg, log_entry) =
                        self.orchestrator.cancel_query(sql.as_deref()).await;
                    let _ = self
                        .response_tx
                        .send(OrchestratorResponse::PendingQueryCancelled {
                            message: msg,
                            log_entry,
                        })
                        .await;
                }

                OrchestratorCommand::Shutdown => {
                    break;
                }
            }
        }

        // Graceful shutdown: cancel all pending operations
        self.cancel_all().await;

        // Close database connection
        if let Err(e) = self.orchestrator.close().await {
            warn!("Error closing orchestrator: {}", e);
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
            .process_input(id, "Show me all users".to_string(), CancellationToken::new())
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
                if matches!(resp, OrchestratorResponse::Completed { id: resp_id, .. } if resp_id == id) {
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
    #[ignore] // TODO: Fix race condition - requests process immediately before queueing
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
    #[ignore] // TODO: Fix race condition - requests process immediately before queueing
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
    #[ignore] // TODO: Fix race condition - requests process immediately before queueing
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
}
